//! 通道的 Windows 那一半（施工 M-3 下，设计 `39-脚本调工具.md` 第五节第 2、3、4 条）：命名管道
//! `\\.\pipe\miyu-call-<随机>`，访问控制开给本人，另给本人沙盒里的命令连的权限（`miyu_pipe::create_for_containers`：沙盒
//! 里的命令跑在 MXC 的基础容器里，令牌是 AppContainer 的，只开给本人的连不上，施工 M-3 下在虚拟机的桌面会话里量过）；照对方的进程编号
//! （`GetNamedPipeClientProcessId`）查它在不在这条命令的作业对象里（`IsProcessInJob`）认人；`PATH` 里的 `miyu.exe` 是核心程序
//! 的硬链接（建符号链接要特权），建不成的把核心所在的目录放到最前面。协议、令牌、等它记下是谁在 `serve.rs`。
//!
//! 连上来的是谁照旧靠令牌和作业对象认：容器里的命令由助手起、放进 MXC 自己的作业对象，那个作业对象嵌在这条命令的里面。

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use tokio::net::windows::named_pipe::NamedPipeServer;
use tokio::task::{AbortHandle, JoinSet};

use miyu_sandbox::Sandboxed;
use miyu_tool::{NestedPort, Process};

use super::Texts;
use super::serve::{self, Binder, Bound, TARGET};

/// 一条命令的端点。丢掉就关。
pub(in crate::shell) struct Channel {
    name: String,
    token: String,
    /// 这条命令的作业对象：起来以后才知道（放到后台的，交给任务表、挂到任务下面以后才记）。
    job: Bound,
    server: AbortHandle,
}

impl Channel {
    /// 照端口 `port` 开一个：建管道的第一个实例、起服务的任务。建不成的交回出错，命令照旧跑，只是没有通道。
    ///
    /// # Errors
    ///
    /// 随机数读不出、管道建不成（名字被占了）。
    pub(in crate::shell) fn open(
        port: Arc<dyn NestedPort>,
        texts: Arc<Texts>,
    ) -> io::Result<Channel> {
        let token = serve::random_hex()?;
        let name = format!(r"\\.\pipe\miyu-call-{}", &token[..16]);
        let first = miyu_pipe::create_for_containers(name.as_ref(), true)?;
        let job: Bound = Arc::new(OnceLock::new());
        let server = tokio::spawn(accept(
            first,
            name.clone(),
            port,
            texts,
            token.clone(),
            Arc::clone(&job),
        ))
        .abort_handle();
        Ok(Channel {
            name,
            token,
            job,
            server,
        })
    }

    /// 记作业对象的那一头：命令起来了（放到后台的，挂到任务下面以后）照它记，之后连上来的照它认。
    pub(in crate::shell) fn binder(&self) -> Binder {
        Binder(Arc::clone(&self.job))
    }

    /// 环境变量：管道的名字、令牌，`PATH` 最前面加上放 `miyu.exe` 的目录。加在挑好的那几个后面。
    pub(in crate::shell) fn env(&self, env: &mut Vec<(OsString, OsString)>) {
        serve::env(env, self.name.clone().into(), &self.token, bin_dir());
    }

    /// 沙盒的规格多一个要连的端点：管道的名字放进 `sockets`，和 Unix 一样。Windows 的助手不看这一格：连不连得上由管道自己的
    /// 访问控制定（`open`）。
    pub(in crate::shell) fn sandboxed(&self, sandboxed: &Sandboxed) -> Sandboxed {
        let mut sandboxed = sandboxed.clone();
        sandboxed.spec.sockets.push(PathBuf::from(&self.name));
        sandboxed
    }
}

/// 丢掉就关：服务的任务掐掉，管道的实例跟着关，名字没了；脚本里等着的 `miyu call` 读到结尾。
impl Drop for Channel {
    fn drop(&mut self) {
        self.server.abort();
    }
}

/// 放到后台的命令带着它的端点交给任务表：任务结束（进程收掉）时一起关。
pub(in crate::shell) fn keep(process: Box<dyn Process>, channel: Channel) -> Box<dyn Process> {
    serve::keep(process, channel)
}

/// 放 `miyu.exe` 的目录：`%TEMP%\miyu-call-<进程号>\bin`，里面是核心程序的硬链接；建不成的（不在同一个盘上）用核心所在的
/// 目录。都拿不到的不加，`miyu call` 照 `PATH` 里原有的找。
fn bin_dir() -> Option<PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let exe = std::env::current_exe().ok()?;
        let dir = std::env::temp_dir()
            .join(format!("miyu-call-{}", std::process::id()))
            .join("bin");
        let linked = std::fs::create_dir_all(&dir).and_then(|()| {
            let link = dir.join("miyu.exe");
            match std::fs::remove_file(&link) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
                _ => {}
            }
            std::fs::hard_link(&exe, &link)
        });
        match linked {
            Ok(()) => Some(dir),
            Err(error) => {
                tracing::debug!(target: TARGET, error = %error, "miyu.exe not linked, using its own directory");
                exe.parent().map(PathBuf::from)
            }
        }
    })
    .clone()
}

/// 服务：等一个连上来，交给一个任务答，接着建下一个实例等。多了的直接断开。
async fn accept(
    first: NamedPipeServer,
    name: String,
    port: Arc<dyn NestedPort>,
    texts: Arc<Texts>,
    token: String,
    job: Bound,
) {
    let mut open = JoinSet::new();
    let mut next = first;
    loop {
        tokio::select! {
            connected = next.connect() => {
                if let Err(error) = connected {
                    tracing::warn!(target: TARGET, error = %error, "miyu call endpoint stopped");
                    break;
                }
                let pipe = match miyu_pipe::create_for_containers(name.as_ref(), false) {
                    Ok(another) => std::mem::replace(&mut next, another),
                    Err(error) => {
                        tracing::warn!(target: TARGET, error = %error, "miyu call endpoint stopped");
                        break;
                    }
                };
                // 多了的：丢掉，对方读到结尾。
                if open.len() >= serve::MAX_OPEN {
                    continue;
                }
                let pid = miyu_pipe::client_process(&pipe).ok();
                let job = Arc::clone(&job);
                let ours = move || ours(pid, job);
                open.spawn(serve::answer(pipe, ours, Arc::clone(&port), Arc::clone(&texts), token.clone()));
            },
            Some(_) = open.join_next(), if !open.is_empty() => {}
        }
    }
}

/// 连上来的进程 `pid` 在不在这条命令的作业对象里。还没记下的等一会儿。
async fn ours(pid: Option<u32>, job: Bound) -> bool {
    let Some(job) = serve::bound(&job).await else {
        return false;
    };
    pid.is_some_and(|pid| job.contains(pid))
}

#[cfg(test)]
mod tests;
