//! 通道的 Windows 那一半（施工 M-3 下，设计 `39-脚本调工具.md` 第五节第 2、3、4 条）：命名管道
//! `\\.\pipe\miyu-call-<随机>`，访问控制开给本人，另给本人沙盒里的命令连的权限（`miyu_pipe::create_for_containers`：沙盒
//! 里的命令跑在 MXC 的基础容器里，令牌是 AppContainer 的，只开给本人的连不上，施工 M-3 下在虚拟机的桌面会话里量过）；照对方的进程编号
//! （`GetNamedPipeClientProcessId`）查它在不在这条命令的作业对象里（`IsProcessInJob`）认人；`PATH` 里的 `miyu.exe` 是核心程序
//! 的硬链接（建符号链接要特权），建不成的把核心所在的目录放到最前面。协议、令牌在 `serve.rs`，认人在 `kin.rs`。
//!
//! `shell` 的命令连上来照旧靠令牌和作业对象认：容器里的命令由助手起、放进 MXC 自己的作业对象，那个作业对象嵌在这条命令的
//! 里面。扩展的命令照进程快照认子孙（施工 XG-1 下）；管道的访问控制两种一样（扩展不进沙盒，开给容器的那一条用不上，也碍不着）。

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use tokio::net::windows::named_pipe::NamedPipeServer;
use tokio::task::{AbortHandle, JoinSet};

use miyu_sandbox::Sandboxed;
use miyu_tool::{NestedPort, Process};

use crate::kin::{Binder, Kin};
use crate::serve::{self, Handed};
use crate::{TARGET, Texts, leftovers};

/// 一次跑命令的端点。丢掉就关。
pub struct Channel {
    name: String,
    token: String,
    /// 认谁：`shell` 的这条命令的作业对象（起来以后才知道，放到后台的交给任务表、挂到任务下面以后才记），或者扩展的子孙。
    kin: Kin,
    server: AbortHandle,
}

impl Channel {
    /// 照端口 `port` 开一个，认 `kin`：建管道的第一个实例、起服务的任务。建不成的交回出错，命令照旧跑，只是没有通道。
    ///
    /// # Errors
    ///
    /// 随机数读不出、管道建不成（名字被占了）。
    pub fn open(port: Arc<dyn NestedPort>, texts: Arc<Texts>, kin: Kin) -> io::Result<Channel> {
        let token = serve::random_hex()?;
        let name = format!(r"\\.\pipe\miyu-call-{}", serve::endpoint_tag()?);
        let first = miyu_pipe::create_for_containers(name.as_ref(), true)?;
        let server = tokio::spawn(accept(
            first,
            name.clone(),
            port,
            texts,
            token.clone(),
            kin.clone(),
        ))
        .abort_handle();
        Ok(Channel {
            name,
            token,
            kin,
            server,
        })
    }

    /// 记作业对象的那一头（只有 `shell` 的 [`Kin::Command`] 有）：命令起来了（放到后台的，挂到任务下面以后）照它记，之后连上来的
    /// 照它认。
    pub fn binder(&self) -> Option<Binder> {
        self.kin.binder()
    }

    /// 环境变量：管道的名字、令牌，`PATH` 最前面加上放 `miyu.exe` 的目录。加在挑好的那几个后面。
    pub fn env(&self, env: &mut Vec<(OsString, OsString)>) {
        serve::env(env, self.name.clone().into(), &self.token, bin_dir());
    }

    /// 交给扩展的那一格（`tool.call` 的 `channel`）：同 [`Channel::env`] 设的几样。
    pub fn handed(&self) -> Option<Handed> {
        serve::handed(self.name.clone().into(), &self.token, bin_dir())
    }

    /// 沙盒的规格多一个要连的端点：管道的名字放进 `sockets`，和 Unix 一样。Windows 的助手不看这一格：连不连得上由管道自己的
    /// 访问控制定（`open`）。
    pub fn sandboxed(&self, sandboxed: &Sandboxed) -> Sandboxed {
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
pub fn keep(process: Box<dyn Process>, channel: Channel) -> Box<dyn Process> {
    serve::keep(process, channel)
}

/// 放 `miyu.exe` 的目录：`%TEMP%\miyu-call-<进程号>\bin`，里面是核心程序的硬链接；建不成的（不在同一个盘上、没有权限：审计
/// 11-03 在虚拟机上看到装在 `Program Files` 里的核心留下的 `bin` 都是空的）用核心所在的目录，建了的 `miyu-call-<进程号>` 当场
/// 删掉，不留空目录。都拿不到的不加，`miyu call` 照 `PATH` 里原有的找。
///
/// 第一次用时先删别的核心进程留下、进程已经不在了的（施工 WC-1，`leftovers.rs`），同一天看到 10-21 以来攒了二十多个。核心走时
/// 不删自己的那一个：链接指着的就是正在跑的程序，Windows 不让删正在跑的程序（没在虚拟机上单独量过），收拾交给下一个核心。不看属主：`%TEMP%` 是每个用户自己的
/// （`%LOCALAPPDATA%\Temp`，别的用户进不来）；指到大家共用的目录里的，别人的目录照继承来的访问控制本来就删不掉，别人的进程
/// 打不开、当还在（`miyu_pipe::process_running`），都不会动到。
fn bin_dir() -> Option<PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let exe = std::env::current_exe().ok()?;
        let temp = std::env::temp_dir();
        sweep(&temp);
        let home = temp.join(format!("{}{}", leftovers::PREFIX, std::process::id()));
        let dir = home.join("bin");
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
                if let Err(error) = std::fs::remove_dir_all(&home)
                    && error.kind() != io::ErrorKind::NotFound
                {
                    tracing::debug!(target: TARGET, error = %error, "unused miyu call directory not removed");
                }
                exe.parent().map(PathBuf::from)
            }
        }
    })
    .clone()
}

/// 删掉临时目录 `temp` 下别的核心进程留下、进程已经不在了的目录（`leftovers.rs`）。不看属主，为什么见 [`bin_dir`]。
fn sweep(temp: &Path) {
    leftovers::sweep(temp, miyu_pipe::process_running, |_| true);
}

/// 同时空着等人连的实例有几个（施工 XG-1补2）。
const IDLE: usize = 2;

/// 服务：一直留 [`IDLE`] 个空着的实例等人连。连上了一个，先建好下一个顶上，再交给一个任务答；多了的直接断开。
///
/// 只留一个的话，连上那一刻到建好下一个之间再来连的拿到 `ERROR_PIPE_BUSY`（231）：`miyu call` 会等着重试，扩展作者自己写的
/// 客户端不一定（施工 XG-1补2，e0 查 Windows 偶发时发现的）。客户端连到哪个空着的实例由系统挑，所以每个都要有人等着：各自
/// 放进一个任务里等，连上了的连同管道一起交回来。
async fn accept(
    first: NamedPipeServer,
    name: String,
    port: Arc<dyn NestedPort>,
    texts: Arc<Texts>,
    token: String,
    kin: Kin,
) {
    let mut open = JoinSet::new();
    let mut idle = JoinSet::new();
    idle.spawn(connected(first));
    for _ in 1..IDLE {
        match miyu_pipe::create_for_containers(name.as_ref(), false) {
            Ok(another) => idle.spawn(connected(another)),
            Err(error) => {
                tracing::warn!(target: TARGET, error = %error, "miyu call endpoint stopped");
                return;
            }
        };
    }
    loop {
        tokio::select! {
            Some(done) = idle.join_next() => {
                let pipe = match done {
                    Ok(Ok(pipe)) => pipe,
                    Ok(Err(error)) => {
                        tracing::warn!(target: TARGET, error = %error, "miyu call endpoint stopped");
                        break;
                    }
                    // 等的任务没了（这个任务正被取消）：收工。
                    Err(_) => break,
                };
                // 先顶上一个空着的，再去答：这一下子再来连的有实例接着。
                match miyu_pipe::create_for_containers(name.as_ref(), false) {
                    Ok(another) => idle.spawn(connected(another)),
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
                let kin = kin.clone();
                let ours = move || async move { kin.ours(pid).await };
                open.spawn(serve::answer(pipe, ours, Arc::clone(&port), Arc::clone(&texts), token.clone()));
            },
            Some(_) = open.join_next(), if !open.is_empty() => {}
            else => break,
        }
    }
}

/// 等一个实例有人连上，连同它交回来。
async fn connected(pipe: NamedPipeServer) -> io::Result<NamedPipeServer> {
    pipe.connect().await.map(|()| pipe)
}

#[cfg(test)]
mod tests;
