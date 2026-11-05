//! 通道的 Unix 那一半（施工 M-3 上）：套接字，对方的进程号照 `SO_PEERCRED` / `LOCAL_PEERPID` 拿，交给 `kin.rs` 认人，`PATH`
//! 里的 `miyu` 是符号链接。协议、令牌在 `serve.rs`。

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use tokio::net::UnixListener;
use tokio::task::{AbortHandle, JoinSet};

use miyu_sandbox::Sandboxed;
use miyu_tool::{NestedPort, Process};

use crate::kin::{Binder, Kin};
use crate::serve::{self, Handed};
#[cfg(test)]
use crate::serve::{MAX_OPEN, MAX_REQUEST, VERSION};
use crate::{TARGET, Texts, leftovers};

/// 一次跑命令的端点。丢掉就关。
pub struct Channel {
    path: PathBuf,
    token: String,
    /// 认谁：`shell` 的这条命令的进程组（起来以后才知道，放到后台的交给任务表、挂到任务下面以后才记），或者扩展的子孙。
    kin: Kin,
    server: AbortHandle,
}

impl Channel {
    /// 照端口 `port` 开一个，认 `kin`：建套接字、起服务的任务。建不成的交回出错，命令照旧跑，只是没有通道。
    ///
    /// # Errors
    ///
    /// 放套接字的目录建不成、随机数读不出、套接字绑不上。
    pub fn open(port: Arc<dyn NestedPort>, texts: Arc<Texts>, kin: Kin) -> io::Result<Channel> {
        let dir = home()?.join("s");
        create_private(&dir)?;
        let token = serve::random_hex()?;
        let path = dir.join(format!("{}.sock", &token[..16]));
        let listener = UnixListener::bind(&path)?;
        let server =
            tokio::spawn(accept(listener, port, texts, token.clone(), kin.clone())).abort_handle();
        Ok(Channel {
            path,
            token,
            kin,
            server,
        })
    }

    /// 记进程组的那一头（只有 `shell` 的 [`Kin::Command`] 有）：命令起来了（放到后台的，挂到任务下面以后）照它记，之后连上来的
    /// 照它认。
    pub fn binder(&self) -> Option<Binder> {
        self.kin.binder()
    }

    /// 环境变量：套接字的路径、令牌，`PATH` 最前面加上放 `miyu` 的目录。加在挑好的那几个后面。
    pub fn env(&self, env: &mut Vec<(OsString, OsString)>) {
        serve::env(
            env,
            self.path.clone().into_os_string(),
            &self.token,
            bin_dir(),
        );
    }

    /// 交给扩展的那一格（`tool.call` 的 `channel`）：同 [`Channel::env`] 设的几样。路径不是 UTF-8 的没有。
    pub fn handed(&self) -> Option<Handed> {
        serve::handed(self.path.clone().into_os_string(), &self.token, bin_dir())
    }

    /// 沙盒的规格放行这个套接字；只准读名单里的（外部身份的会话），再放开读 `miyu` 和它指着的程序。核心的程序被藏起来的盖住了
    /// 的（私密目录包住了装的位置），单独只读放开，`miyu call` 才起得来（施工 5-15 下再修）。
    pub fn sandboxed(&self, sandboxed: &Sandboxed) -> Sandboxed {
        let mut sandboxed = sandboxed.clone();
        sandboxed.spec.sockets.push(self.path.clone());
        let exe = std::env::current_exe().ok();
        if let Some(read) = sandboxed.spec.read.as_mut() {
            read.extend(bin_dir());
            read.extend(exe.clone());
        }
        if let Some(exe) = exe
            && sandboxed.spec.hides(&exe)
        {
            sandboxed.spec.reveal.push(exe);
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

/// 放到后台的命令带着它的端点交给任务表：任务结束（进程收掉）时一起关。
pub fn keep(process: Box<dyn Process>, channel: Channel) -> Box<dyn Process> {
    serve::keep(process, channel)
}

/// 这个核心进程的那一个目录：临时目录下，`miyu-call-<进程号>`，只有本人进得去。第一次用时建，顺手删掉别的核心进程留下、
/// 进程已经不在了的。
fn home() -> io::Result<PathBuf> {
    static HOME: OnceLock<Option<PathBuf>> = OnceLock::new();
    HOME.get_or_init(|| {
        let temp = std::env::temp_dir();
        sweep(&temp);
        let dir = temp.join(format!("{}{}", leftovers::PREFIX, std::process::id()));
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

/// 删掉别的核心进程留下、进程已经不在了的目录（`leftovers.rs`）：临时目录几个用户共用，只认属主是自己的。进程号是别的用户的
/// 进程重用了的（问它报 `EPERM`）也算不在了：目录是自己的，那个核心早走了。
fn sweep(temp: &Path) {
    use std::os::unix::fs::MetadataExt;
    let me = rustix::process::getuid().as_raw();
    leftovers::sweep(
        temp,
        |pid| {
            // 放不进 `pid_t` 的、0 不是哪个进程：不碰。
            i32::try_from(pid)
                .ok()
                .and_then(rustix::process::Pid::from_raw)
                .is_none_or(|pid| rustix::process::test_kill_process(pid).is_ok())
        },
        |meta| meta.uid() == me,
    );
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
async fn accept(
    listener: UnixListener,
    port: Arc<dyn NestedPort>,
    texts: Arc<Texts>,
    token: String,
    kin: Kin,
) {
    let mut open = JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) if open.len() < serve::MAX_OPEN => {
                    let pid = stream
                        .peer_cred()
                        .ok()
                        .and_then(|cred| cred.pid())
                        .and_then(|pid| u32::try_from(pid).ok());
                    let kin = kin.clone();
                    let ours = move || async move { kin.ours(pid).await };
                    open.spawn(serve::answer(stream, ours, Arc::clone(&port), Arc::clone(&texts), token.clone()));
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

#[cfg(test)]
mod tests;
