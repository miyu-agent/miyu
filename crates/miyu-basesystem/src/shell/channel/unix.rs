//! 通道的 Unix 那一半（施工 M-3 上）：套接字、令牌、进程组检查、一问一答。

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::task::{AbortHandle, JoinSet};

use miyu_sandbox::Sandboxed;
use miyu_tool::{Exit, NestedPort, Process};

use super::Texts;
use super::reply::{self, Reply, USAGE};
use crate::load::say;

/// 告诉 `miyu call` 套接字在哪的环境变量。
const SOCK_VAR: &str = "MIYU_CALL_SOCK";

/// 令牌的环境变量。
const TOKEN_VAR: &str = "MIYU_CALL_TOKEN";

/// 协议版本：请求带它，对不上的回一句、退出码 2。
const VERSION: u32 = 1;

/// 请求帧的上限。
const MAX_REQUEST: u32 = 1 << 20;

/// 同一个端点同时连着的上限。
const MAX_OPEN: usize = 32;

/// 命令刚起、进程组还没记下时，连上来的最多等多久。
const BIND_WAIT: Duration = Duration::from_secs(5);

/// 运行日志的来处，和 `shell` 的别处一样。
const TARGET: &str = "miyu::shell";

/// 一条命令的端点。丢掉就关。
pub(in crate::shell) struct Channel {
    path: PathBuf,
    token: String,
    /// 这条命令的进程组：起来以后才知道（放到后台的，交给任务表、挂到任务下面以后才记）。
    group: Arc<OnceLock<u32>>,
    server: AbortHandle,
}

impl Channel {
    /// 照端口 `port` 开一个：建套接字、起服务的任务。建不成的交回出错，命令照旧跑，只是没有通道。
    ///
    /// # Errors
    ///
    /// 放套接字的目录建不成、随机数读不出、套接字绑不上。
    pub(in crate::shell) fn open(
        port: Arc<dyn NestedPort>,
        texts: Arc<Texts>,
    ) -> io::Result<Channel> {
        let dir = home()?.join("s");
        create_private(&dir)?;
        let token = random_hex()?;
        let path = dir.join(format!("{}.sock", &token[..16]));
        let listener = UnixListener::bind(&path)?;
        let group = Arc::new(OnceLock::new());
        let server = tokio::spawn(serve(
            listener,
            port,
            texts,
            token.clone(),
            Arc::clone(&group),
        ))
        .abort_handle();
        Ok(Channel {
            path,
            token,
            group,
            server,
        })
    }

    /// 记进程组的那一头：命令起来了（放到后台的，挂到任务下面以后）照它记，之后连上来的照它认。
    pub(in crate::shell) fn binder(&self) -> Binder {
        Binder(Arc::clone(&self.group))
    }

    /// 环境变量：套接字的路径、令牌，`PATH` 最前面加上放 `miyu` 的目录。加在挑好的那几个后面。
    pub(in crate::shell) fn env(&self, env: &mut Vec<(OsString, OsString)>) {
        env.push((SOCK_VAR.into(), self.path.clone().into_os_string()));
        env.push((TOKEN_VAR.into(), self.token.clone().into()));
        let Some(dir) = bin_dir() else {
            return;
        };
        match env.iter_mut().find(|(name, _)| name == "PATH") {
            Some((_, path)) => {
                let mut joined = dir.into_os_string();
                joined.push(":");
                joined.push(&*path);
                *path = joined;
            }
            None => env.push(("PATH".into(), dir.into_os_string())),
        }
    }

    /// 沙盒的规格放行这个套接字；只准读名单里的（外部身份的会话），再放开读 `miyu` 和它指着的程序。
    pub(in crate::shell) fn sandboxed(&self, sandboxed: &Sandboxed) -> Sandboxed {
        let mut sandboxed = sandboxed.clone();
        sandboxed.spec.sockets.push(self.path.clone());
        if let Some(read) = sandboxed.spec.read.as_mut() {
            read.extend(bin_dir());
            read.extend(std::env::current_exe().ok());
        }
        sandboxed
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        self.server.abort();
        if let Err(error) = std::fs::remove_file(&self.path) {
            tracing::debug!(target: TARGET, error = %error, "miyu call socket not removed");
        }
    }
}

/// 记一条命令的进程组。
pub(in crate::shell) struct Binder(Arc<OnceLock<u32>>);

impl Binder {
    /// 记下进程组 `group`：只记一次。
    pub(in crate::shell) fn bind(self, group: u32) {
        if self.0.set(group).is_err() {
            tracing::debug!(target: TARGET, "miyu call endpoint bound twice");
        }
    }
}

/// 放到后台的命令带着它的端点交给任务表：任务结束（进程收掉）时一起关。
pub(in crate::shell) fn keep(process: Box<dyn Process>, channel: Channel) -> Box<dyn Process> {
    Box::new(Kept {
        process,
        _channel: channel,
    })
}

struct Kept {
    process: Box<dyn Process>,
    _channel: Channel,
}

impl Process for Kept {
    fn wait(&self) -> io::Result<Exit> {
        self.process.wait()
    }

    fn kill(&self) {
        self.process.kill();
    }
}

/// 这个核心进程的那一个目录：临时目录下，`miyu-call-<进程号>`，只有本人进得去。第一次用时建，顺手删掉别的核心进程留下、
/// 进程已经不在了的。
fn home() -> io::Result<PathBuf> {
    static HOME: OnceLock<Option<PathBuf>> = OnceLock::new();
    HOME.get_or_init(|| {
        let temp = std::env::temp_dir();
        sweep(&temp);
        let dir = temp.join(format!("miyu-call-{}", std::process::id()));
        create_private(&dir).ok().map(|()| dir)
    })
    .clone()
    .ok_or_else(|| io::Error::other("no private directory for miyu call"))
}

/// 建目录，权限 `0700`。
fn create_private(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

/// 删掉别的核心进程留下、进程已经不在了的目录：只认自己的、名字对得上的。
fn sweep(temp: &Path) {
    use std::os::unix::fs::MetadataExt;
    let Ok(entries) = std::fs::read_dir(temp) else {
        return;
    };
    let me = rustix::process::getuid().as_raw();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name
            .to_str()
            .and_then(|name| name.strip_prefix("miyu-call-"))
            .and_then(|pid| pid.parse::<i32>().ok())
        else {
            continue;
        };
        let mine = entry
            .metadata()
            .is_ok_and(|meta| meta.is_dir() && meta.uid() == me);
        let gone = rustix::process::Pid::from_raw(pid)
            .is_some_and(|pid| rustix::process::test_kill_process(pid).is_err());
        if mine
            && gone
            && let Err(error) = std::fs::remove_dir_all(entry.path())
        {
            tracing::debug!(target: TARGET, error = %error, "stale miyu call directory not removed");
        }
    }
}

/// 32 个十六进制字符的随机串：128 位。
fn random_hex() -> io::Result<String> {
    use std::io::Read;
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// 放 `miyu` 的目录：只有这一个链接，指向核心自己。建不成的不加，`miyu call` 照 `PATH` 里原有的找。
fn bin_dir() -> Option<PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let exe = std::env::current_exe().ok()?;
        let dir = home().ok()?.join("bin");
        create_private(&dir).ok()?;
        let link = dir.join("miyu");
        // 上一次留下的换成指向现在这个程序的：不在的不算出错。
        match std::fs::remove_file(&link) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return None,
            _ => {}
        }
        std::os::unix::fs::symlink(&exe, &link).ok()?;
        Some(dir)
    })
    .clone()
}

/// 服务：接连上来的，各起一个任务答。多了的直接断开。
async fn serve(
    listener: UnixListener,
    port: Arc<dyn NestedPort>,
    texts: Arc<Texts>,
    token: String,
    group: Arc<OnceLock<u32>>,
) {
    let mut open = JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) if open.len() < MAX_OPEN => {
                    open.spawn(answer(stream, Arc::clone(&port), Arc::clone(&texts), token.clone(), Arc::clone(&group)));
                }
                // 多了的：丢掉，对方读到结尾。
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(target: TARGET, error = %error, "miyu call endpoint stopped");
                    break;
                }
            },
            Some(_) = open.join_next(), if !open.is_empty() => {}
        }
    }
}

/// 连上来的进程在不在这条命令的进程组里。组还没记下的等一会儿。
async fn ours(stream: &UnixStream, group: &OnceLock<u32>) -> bool {
    let waited = tokio::time::timeout(BIND_WAIT, async {
        loop {
            if let Some(&group) = group.get() {
                return group;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    let Ok(group) = waited else {
        return false;
    };
    let Some(pid) = stream
        .peer_cred()
        .ok()
        .and_then(|cred| cred.pid())
        .and_then(rustix::process::Pid::from_raw)
    else {
        return false;
    };
    rustix::process::getpgid(Some(pid))
        .is_ok_and(|pgid| u32::try_from(pgid.as_raw_nonzero().get()) == Ok(group))
}

/// 一次请求：协议版本、做什么（`call`、`schema`）、工具名、参数原文、令牌。
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
    mut stream: UnixStream,
    port: Arc<dyn NestedPort>,
    texts: Arc<Texts>,
    token: String,
    group: Arc<OnceLock<u32>>,
) {
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
    let refused = |template| Reply::said(USAGE, say(template, &[]));
    let reply = match serde_json::from_slice::<Request>(&body) {
        Err(_) => refused(&texts.bad_request),
        Ok(request) if request.token != token || !ours(&stream, &group).await => {
            refused(&texts.stranger)
        }
        Ok(request) if request.v != VERSION => refused(&texts.version),
        Ok(request) => match request.op.as_str() {
            "call" => reply::call(&*port, &texts, request.tool, request.args).await,
            "schema" => reply::schema(&*port, &texts, &request.tool),
            _ => refused(&texts.bad_request),
        },
    };
    let body = serde_json::json!({
        "code": reply.code,
        "stdout": reply.stdout,
        "stderr": reply.stderr,
    })
    .to_string();
    // 写不进去的：对方已经不在了（脚本被杀），结果没人要。
    if let Err(error) = send(&mut stream, &body).await {
        tracing::debug!(target: TARGET, error = %error, "miyu call reply not sent");
    }
}

/// 写一帧：4 字节大端长度加字。
async fn send(stream: &mut UnixStream, body: &str) -> io::Result<()> {
    let len = u32::try_from(body.len()).map_err(io::Error::other)?;
    stream.write_u32(len).await?;
    stream.write_all(body.as_bytes()).await?;
    stream.flush().await
}

#[cfg(test)]
mod tests;
