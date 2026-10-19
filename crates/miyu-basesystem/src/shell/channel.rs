//! 脚本调工具的通道（设计 39 第五节，原型 M-0）：前台的 `shell` 起命令前建，命令跑完就关。
//!
//! - 入口：一对 `SOCK_DGRAM`，子进程那一头放到编号 3（`MIYU_CALL_FD=3`）。`miyu call` 每次自己建一对流，一头经入口
//!   用 `SCM_RIGHTS` 发过来，一个数据报带一个描述符，并发的几个不会搅在一起。
//! - 私有的流上：4 字节大端长度加 UTF-8 JSON，一次连接一个请求、一个回答。
//! - 另一头不可信：数据报不带恰好一个描述符的丢掉；请求帧超过 1 MiB 的断开；同时连着的超过 32 个的直接断开。
//! - `PATH` 最前面加一个只放 `miyu` 链接的目录，指向核心自己：脚本里调到的一定是同一个版本。
//! - 丢掉 [`Channel`] 就关：服务的任务掐掉，入口和所有私有的流跟着关，脚本里等着的 `miyu call` 读到结尾。

use std::ffi::OsString;
use std::io::{self, IoSliceMut};
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::os::unix::net::{UnixDatagram, UnixStream};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use rustix::net::{RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, recvmsg};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::io::Interest;
use tokio::task::{AbortHandle, JoinSet};

use miyu_tool::{NestedPort, Outcome};

/// 子进程里通道入口的编号。
pub(super) const FD: RawFd = 3;

/// 告诉 `miyu call` 入口在哪个编号上的环境变量。
const FD_VAR: &str = "MIYU_CALL_FD";

/// 协议版本：第一个帧带它，对不上的回一句、退出码 2。
const VERSION: u32 = 1;

/// 请求帧的上限。
const MAX_REQUEST: u32 = 1 << 20;

/// 同一次 shell 调用同时连着的上限。
const MAX_OPEN: usize = 32;

/// 一次 shell 调用的通道。丢掉就关。
pub(super) struct Channel {
    /// 交给子进程的那一头：起完命令就关掉父进程里的这一份。
    child: Option<UnixDatagram>,
    server: AbortHandle,
}

impl Channel {
    /// 照 `port` 开一条：建入口，起服务的任务。
    pub(super) fn open(port: Arc<dyn NestedPort>) -> io::Result<Channel> {
        let (core, child) = UnixDatagram::pair()?;
        core.set_nonblocking(true)?;
        let core = tokio::net::UnixDatagram::from_std(core)?;
        let server = tokio::spawn(serve(core, port)).abort_handle();
        Ok(Channel {
            child: Some(child),
            server,
        })
    }

    /// 交给子进程的那一头的编号：起命令前照它放到 [`FD`]。
    pub(super) fn child_fd(&self) -> Option<RawFd> {
        self.child.as_ref().map(AsRawFd::as_raw_fd)
    }

    /// 起完命令：关掉父进程里那一头的这一份。
    pub(super) fn started(&mut self) {
        self.child = None;
    }

    /// 环境变量：入口的编号，`PATH` 最前面加上放 `miyu` 链接的目录。
    pub(super) fn env(env: &mut Vec<(OsString, OsString)>) {
        env.push((FD_VAR.into(), FD.to_string().into()));
        let Some(dir) = bin_dir() else {
            return;
        };
        match env.iter_mut().find(|(name, _)| name == "PATH") {
            Some((_, path)) => {
                let mut joined = dir.clone().into_os_string();
                joined.push(":");
                joined.push(&*path);
                *path = joined;
            }
            None => env.push(("PATH".into(), dir.into_os_string())),
        }
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        self.server.abort();
    }
}

/// 放 `miyu` 链接的目录：一个核心进程一个，第一次用时建。建不成的不加，`miyu call` 照 `PATH` 里原有的找。
fn bin_dir() -> Option<PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let exe = std::env::current_exe().ok()?;
        let dir = std::env::temp_dir().join(format!("miyu-call-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok()?;
        let link = dir.join("miyu");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&exe, &link).ok()?;
        Some(dir)
    })
    .clone()
}

/// 服务：收入口上发来的描述符，一个一个接着答。入口读到结尾、出错就停。
async fn serve(core: tokio::net::UnixDatagram, port: Arc<dyn NestedPort>) {
    let mut open = JoinSet::new();
    loop {
        tokio::select! {
            received = core.async_io(Interest::READABLE, || receive(&core)) => match received {
                Ok(Some(stream)) if open.len() < MAX_OPEN => {
                    open.spawn(answer(stream, Arc::clone(&port)));
                }
                // 多了的、不带描述符的：丢掉，对方读到结尾。
                Ok(_) => {}
                Err(_) => break,
            },
            Some(_) = open.join_next(), if !open.is_empty() => {}
        }
    }
}

/// 收一个数据报：恰好带一个描述符的交回它，别的交回 `None`。
fn receive(core: &tokio::net::UnixDatagram) -> io::Result<Option<OwnedFd>> {
    let mut space = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(2))];
    let mut ancillary = RecvAncillaryBuffer::new(&mut space);
    let mut byte = [0u8; 16];
    let got = recvmsg(
        core,
        &mut [IoSliceMut::new(&mut byte)],
        &mut ancillary,
        RecvFlags::CMSG_CLOEXEC,
    )
    .map_err(io::Error::from)?;
    if got.bytes == 0 && ancillary.drain().next().is_none() {
        return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
    }
    let mut fds: Vec<OwnedFd> = Vec::new();
    for message in ancillary.drain() {
        if let RecvAncillaryMessage::ScmRights(received) = message {
            fds.extend(received);
        }
    }
    Ok(match fds.len() {
        1 => fds.pop(),
        _ => None,
    })
}

/// 一次请求。
#[derive(Deserialize)]
struct Request {
    v: u32,
    op: String,
    tool: String,
    #[serde(default)]
    args: String,
}

/// 在私有的流 `fd` 上答一个请求。读写出错就丢掉：对方已经不在了。
async fn answer(fd: OwnedFd, port: Arc<dyn NestedPort>) {
    let std = UnixStream::from(fd);
    if std.set_nonblocking(true).is_err() {
        return;
    }
    let Ok(mut stream) = tokio::net::UnixStream::from_std(std) else {
        return;
    };
    let Ok(len) = stream.read_u32().await else {
        return;
    };
    if len > MAX_REQUEST {
        return;
    }
    let mut body = vec![0u8; len as usize];
    if stream.read_exact(&mut body).await.is_err() {
        return;
    }
    let outcome = match serde_json::from_slice::<Request>(&body) {
        Err(_) => Outcome::refused(2, "Bad request from miyu call."),
        Ok(request) if request.v != VERSION => Outcome::refused(
            2,
            "This miyu call does not match the running Miyu. Use the miyu found first on PATH.",
        ),
        Ok(request) => match request.op.as_str() {
            "call" => port.call(request.tool, request.args).await,
            "schema" => match port.schema(&request.tool) {
                Some(text) => Outcome {
                    code: 0,
                    stdout: text,
                    stderr: String::new(),
                },
                None => Outcome::refused(2, format!("miyu call does not work for {}.", request.tool)),
            },
            _ => Outcome::refused(2, "Bad request from miyu call."),
        },
    };
    let reply = serde_json::json!({
        "code": outcome.code,
        "stdout": outcome.stdout,
        "stderr": outcome.stderr,
    })
    .to_string();
    let Ok(len) = u32::try_from(reply.len()) else {
        return;
    };
    let _ = stream.write_u32(len).await;
    let _ = stream.write_all(reply.as_bytes()).await;
    let _ = stream.flush().await;
}
