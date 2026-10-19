//! 脚本调工具的通道（设计 39 第五节，原型 M-0）：前台的 `shell` 起命令前建，命令跑完就关。
//!
//! - 入口：一对 `SOCK_DGRAM`，子进程那一头放到编号 3（`MIYU_CALL_FD=3`）。`miyu call` 每次自己建一对流，一头经入口
//!   用 `SCM_RIGHTS` 发过来，一个数据报带一个描述符，并发的几个不会搅在一起。
//! - 私有的流上：4 字节大端长度加 UTF-8 JSON，一次连接一个请求、一个回答。
//! - 另一头不可信：数据报不带恰好一个描述符的丢掉；请求帧超过 1 MiB 的断开；同时连着的超过 32 个的直接断开。
//! - `PATH` 最前面加一个只放 `miyu` 链接的目录，指向核心自己：脚本里调到的一定是同一个版本。
//! - 丢掉 [`Channel`] 就关：服务的任务掐掉，入口和所有私有的流跟着关，脚本里等着的 `miyu call` 读到结尾。

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::{AbortHandle, JoinSet};

use miyu_tool::{NestedPort, Outcome};

/// 告诉 `miyu call` 套接字在哪的环境变量。
const SOCK_VAR: &str = "MIYU_CALL_SOCK";

/// 令牌的环境变量。
const TOKEN_VAR: &str = "MIYU_CALL_TOKEN";

/// 协议版本：第一个帧带它，对不上的回一句、退出码 2。
const VERSION: u32 = 1;

/// 请求帧的上限。
const MAX_REQUEST: u32 = 1 << 20;

/// 同一次 shell 调用同时连着的上限。
const MAX_OPEN: usize = 32;

/// 一次 shell 调用的通道。丢掉就关。
pub(super) struct Channel {
    /// 套接字的路径：环境变量交给子进程，子进程再起的子进程照样继承到（Python、Node 默认关掉多余的描述符，编号 3 会断）。
    path: PathBuf,
    /// 一次性的令牌：连上的先出示，对不上的不答。
    token: String,
    /// 这条命令的进程组：起来以后才知道。连上来的进程不在这一组的不答（令牌漏了也只有这条命令自己的进程能用）。
    group: Arc<OnceLock<u32>>,
    server: AbortHandle,
}

impl Channel {
    /// 照 `port` 开一条：建套接字，起服务的任务。
    pub(super) fn open(port: Arc<dyn NestedPort>) -> io::Result<Channel> {
        let dir = sock_dir().ok_or_else(|| io::Error::other("no socket dir"))?;
        let token = random_hex()?;
        let path = dir.join(format!("{}.sock", &token[..16]));
        let listener = tokio::net::UnixListener::bind(&path)?;
        let group = Arc::new(OnceLock::new());
        let server = tokio::spawn(serve(listener, port, token.clone(), Arc::clone(&group))).abort_handle();
        Ok(Channel {
            path,
            token,
            group,
            server,
        })
    }

    /// 命令起来了：记下它的进程组。
    pub(super) fn bind(&self, group: u32) {
        let _ = self.group.set(group);
    }

    /// 环境变量：套接字的路径、令牌，`PATH` 最前面加上放 `miyu` 链接的目录。
    pub(super) fn env(&self, env: &mut Vec<(OsString, OsString)>) {
        env.push((SOCK_VAR.into(), self.path.clone().into_os_string()));
        env.push((TOKEN_VAR.into(), self.token.clone().into()));
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
        let _ = std::fs::remove_file(&self.path);
    }
}

/// 放套接字的目录：一个核心进程一个，只有本人能进。
fn sock_dir() -> Option<PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("miyu-call-{}", std::process::id())).join("s");
        std::fs::create_dir_all(&dir).ok()?;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).ok()?;
        Some(dir)
    })
    .clone()
}

/// 32 个十六进制字符的随机串。
fn random_hex() -> io::Result<String> {
    use std::io::Read;
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
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

/// 服务：接连上来的，一个一个答。出错就停。
async fn serve(
    listener: tokio::net::UnixListener,
    port: Arc<dyn NestedPort>,
    token: String,
    group: Arc<OnceLock<u32>>,
) {
    let mut open = JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) if open.len() < MAX_OPEN => {
                    open.spawn(answer(stream, Arc::clone(&port), token.clone(), Arc::clone(&group)));
                }
                // 多了的：丢掉，对方读到结尾。
                Ok(_) => {}
                Err(_) => break,
            },
            Some(_) = open.join_next(), if !open.is_empty() => {}
        }
    }
}

/// 连上来的进程在不在这条命令的进程组里。命令刚起、组还没记下的，等一会儿（最多一秒）。
async fn ours(stream: &tokio::net::UnixStream, group: &OnceLock<u32>) -> bool {
    let mut waited = 0;
    let group = loop {
        if let Some(&group) = group.get() {
            break group;
        }
        if waited >= 100 {
            return false;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        waited += 1;
    };
    let Some(pid) = stream.peer_cred().ok().and_then(|cred| cred.pid()) else {
        return false;
    };
    let Some(pid) = rustix::process::Pid::from_raw(pid) else {
        return false;
    };
    rustix::process::getpgid(Some(pid)).is_ok_and(|pgid| pgid.as_raw_nonzero().get().unsigned_abs() == group)
}

/// 一次请求。
#[derive(Deserialize)]
struct Request {
    v: u32,
    op: String,
    tool: String,
    #[serde(default)]
    args: String,
    #[serde(default)]
    token: String,
}

/// 在连上来的流上答一个请求。读写出错就丢掉：对方已经不在了。
async fn answer(
    mut stream: tokio::net::UnixStream,
    port: Arc<dyn NestedPort>,
    token: String,
    group: Arc<OnceLock<u32>>,
) {
    // 不是这条命令的进程：不答，对方读到结尾。
    if !ours(&stream, &group).await {
        return;
    }
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
        Ok(request) if request.token != token => Outcome::refused(2, "miyu call only works inside a command Miyu runs."),
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
