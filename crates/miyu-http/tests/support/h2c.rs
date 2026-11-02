//! 测试用的 HTTP/2 假服务器（施工 V-38）：明文、事先知道是 h2 的那种（h2c），直接读写帧，照剧本一个流一个流地回：拒掉
//! （`REFUSED_STREAM`）、整条连接不收了（`GOAWAY`，`NO_ERROR`，这个流不算）、照常回 200 的事件流、请求体还没收完就回一个出错的
//! 状态（不给对方开窗口，对方最多送 64 KiB 不到）。收到的每个流的请求体都记下来。HPACK 只用静态表，不解请求头。

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// 一个流怎么回。
#[derive(Debug, Clone)]
pub enum Step {
    /// `RST_STREAM`，`REFUSED_STREAM`。
    Refuse,
    /// `GOAWAY`，最后一个流是它前面那个，`NO_ERROR`：这个流对方没处理。
    GoAway,
    /// 收完请求体，回 200 和这些字节，收尾。
    Reply(Vec<u8>),
    /// 一收到请求头就回这个状态、收尾，不给对方开窗口。
    Early(u16),
}

/// 一个流收到的。
#[derive(Debug, Clone, Default)]
pub struct Got {
    /// 请求体。
    pub body: Vec<u8>,
    /// 收完了（带着结束标记）。
    pub complete: bool,
}

/// 跑着的假服务器。
pub struct H2c {
    /// `http://127.0.0.1:<端口>/v1`。
    pub base_url: String,
    got: Arc<Mutex<Vec<Got>>>,
}

impl H2c {
    /// 起一个，照剧本 `steps` 一个流一个流地回（跨连接接着往下）。
    ///
    /// # Panics
    ///
    /// 绑不了端口。
    pub async fn start(steps: Vec<Step>) -> H2c {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("绑得上");
        let port = listener.local_addr().expect("有地址").port();
        let steps = Arc::new(Mutex::new(VecDeque::from(steps)));
        let got = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&got);
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                let steps = Arc::clone(&steps);
                let log = Arc::clone(&log);
                tokio::spawn(async move {
                    drop(serve(socket, &steps, &log).await);
                });
            }
        });
        H2c {
            base_url: format!("http://127.0.0.1:{port}/v1"),
            got,
        }
    }

    /// 收到的，照流开的先后。
    pub fn got(&self) -> Vec<Got> {
        self.got
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// 写一帧。
async fn frame(
    socket: &mut TcpStream,
    kind: u8,
    flags: u8,
    stream: u32,
    payload: &[u8],
) -> std::io::Result<()> {
    let len = u32::try_from(payload.len()).expect("帧不大");
    let mut head = [0u8; 9];
    head[..3].copy_from_slice(&len.to_be_bytes()[1..]);
    head[3] = kind;
    head[4] = flags;
    head[5..].copy_from_slice(&stream.to_be_bytes());
    socket.write_all(&head).await?;
    socket.write_all(payload).await
}

/// 照剧本回一条连接。
async fn serve(
    mut socket: TcpStream,
    steps: &Mutex<VecDeque<Step>>,
    log: &Mutex<Vec<Got>>,
) -> std::io::Result<()> {
    let mut preface = [0u8; 24];
    socket.read_exact(&mut preface).await?;
    // 服务端的开场：一个空的 SETTINGS。
    frame(&mut socket, 0x4, 0, 0, &[]).await?;
    // 流号 → 在 `log` 里的第几个；要收完再回的另记怎么回。
    let mut index_of: BTreeMap<u32, usize> = BTreeMap::new();
    let mut open: BTreeMap<u32, Step> = BTreeMap::new();
    // 早回了的流：连接的窗口也不开，对方最多送 64 KiB 不到。
    let mut early: Vec<u32> = Vec::new();
    loop {
        let mut head = [0u8; 9];
        socket.read_exact(&mut head).await?;
        let len =
            usize::try_from(u32::from_be_bytes([0, head[0], head[1], head[2]])).expect("长度");
        let (kind, flags) = (head[3], head[4]);
        let stream = u32::from_be_bytes([head[5], head[6], head[7], head[8]]) & 0x7fff_ffff;
        let mut payload = vec![0u8; len];
        socket.read_exact(&mut payload).await?;
        let end = flags & 0x1 != 0;
        match kind {
            // SETTINGS：不是回执的回一个回执。
            0x4 if flags & 0x1 == 0 => frame(&mut socket, 0x4, 0x1, 0, &[]).await?,
            // PING：回执带原样的数据。
            0x6 if flags & 0x1 == 0 => frame(&mut socket, 0x6, 0x1, 0, &payload).await?,
            0x1 => {
                let step = steps
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .pop_front()
                    .unwrap_or(Step::Refuse);
                let index = {
                    let mut log = log.lock().unwrap_or_else(PoisonError::into_inner);
                    log.push(Got {
                        body: Vec::new(),
                        complete: end,
                    });
                    log.len() - 1
                };
                index_of.insert(stream, index);
                match step {
                    Step::Refuse => {
                        frame(&mut socket, 0x3, 0, stream, &7u32.to_be_bytes()).await?;
                    }
                    Step::GoAway => {
                        let mut away = (stream.saturating_sub(2)).to_be_bytes().to_vec();
                        away.extend_from_slice(&0u32.to_be_bytes());
                        frame(&mut socket, 0x7, 0, 0, &away).await?;
                    }
                    Step::Early(status) => {
                        early.push(stream);
                        let status = status.to_string();
                        // 字面量、不进表、名字用静态表第 8 个（`:status`）。
                        let mut block = vec![0x08, u8::try_from(status.len()).expect("短")];
                        block.extend_from_slice(status.as_bytes());
                        frame(&mut socket, 0x1, 0x4 | 0x1, stream, &block).await?;
                    }
                    Step::Reply(_) if end => reply(&mut socket, stream, &step).await?,
                    Step::Reply(_) => {
                        open.insert(stream, step);
                    }
                }
            }
            0x0 => {
                if let Some(index) = index_of.get(&stream) {
                    let mut log = log.lock().unwrap_or_else(PoisonError::into_inner);
                    log[*index].body.extend_from_slice(&payload);
                    log[*index].complete = end;
                }
                // 收完再回的流：连接、流的窗口都开；拒了的只开连接的（拒了的流上送过来的也占连接的窗口）；早回了的都不开。
                if !payload.is_empty() && !early.contains(&stream) {
                    let more = u32::try_from(payload.len()).expect("帧不大").to_be_bytes();
                    frame(&mut socket, 0x8, 0, 0, &more).await?;
                    if open.contains_key(&stream) {
                        frame(&mut socket, 0x8, 0, stream, &more).await?;
                    }
                }
                if end && let Some(step) = open.remove(&stream) {
                    reply(&mut socket, stream, &step).await?;
                }
            }
            _ => {}
        }
    }
}

/// 回 200（静态表第 8 项 `:status: 200`）和事件流，收尾。
async fn reply(socket: &mut TcpStream, stream: u32, step: &Step) -> std::io::Result<()> {
    let Step::Reply(body) = step else {
        return Ok(());
    };
    frame(socket, 0x1, 0x4, stream, &[0x88]).await?;
    frame(socket, 0x0, 0x1, stream, body).await
}
