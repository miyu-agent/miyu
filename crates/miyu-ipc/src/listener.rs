//! 在套接字上等连接的一头，和连上以后的一个连接。和平台有关的在 `unix`、`windows`（命名管道），这里只是
//! 一层壳：协议端点只认异步的字节流，不管它从哪来。

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use miyu_store::root::DataRoot;

use crate::lock::{Held, Lock};
use crate::{files, sys};

/// 核心在套接字上等连接（Windows 上是命名管道）。丢掉它：套接字文件、`run/socket` 删掉，放在这个数据根专用的目录里的
/// 连目录一起删，锁放开。先删再放锁：放了锁，下一个核心马上就可能在同一个位置上绑。
///
/// 绑好了还不让头找到（施工 CS-6）：`run/socket` 到 [`Listener::publish`] 才写，核心在开始接连接的前一刻调；原来绑好就写，
/// 核心走到接连接那一步以前（读配置、造路由这些），头连得上却没人回。停的时候锁可以先拿走（[`Listener::take_lock`]），丢掉监听器
/// 只关监听、删文件，锁等收尾做完再放。
pub struct Listener {
    /// 在等连接的套接字。
    socket: sys::Socket,
    /// 套接字在哪。
    path: PathBuf,
    /// 套接字所在的这一层是这个数据根专用的（`$XDG_RUNTIME_DIR/miyu-<指纹>/`）：走的时候空了就删。
    own_dir: Option<PathBuf>,
    /// 数据根：`run/socket` 写在它里面。
    root: DataRoot,
    /// 锁文件在哪、是哪一个（施工 TMP-2）：锁拿走了也照样看得到。
    held: Held,
    /// 单实例锁。放在最后：字段照声明的先后丢，锁最后放。拿走了的（[`Listener::take_lock`]）是空的。
    lock: Option<Lock>,
}

impl Listener {
    /// 套接字绑好了，锁拿着了，`root` 是它的数据根。
    pub(crate) fn new(
        socket: sys::Socket,
        (path, own_dir): (PathBuf, Option<PathBuf>),
        root: DataRoot,
        lock: Lock,
    ) -> Listener {
        Listener {
            socket,
            path,
            own_dir,
            root,
            held: lock.held(),
            lock: Some(lock),
        }
    }

    /// 让头找到它（施工 CS-6）：把实际位置写进 `run/socket`，记一行 `INFO listening`。核心在开始接连接的前一刻调：头照它连上就有人回。
    ///
    /// # Errors
    ///
    /// 写不进。
    pub fn publish(&self) -> io::Result<()> {
        files::write_location(&self.root, &self.path)?;
        tracing::info!(target: "miyu::ipc", socket = %self.path.display(), "listening");
        Ok(())
    }

    /// 把单实例锁拿走、另外拿着（施工 CS-6）：之后丢掉监听器只关监听、删文件，不放锁。拿过一次的再拿是空的。
    pub fn take_lock(&mut self) -> Option<Lock> {
        self.lock.take()
    }

    /// 等下一个连接。Windows 上接走一个连接，要把等着的实例换成新建的，所以要 `&mut`。
    ///
    /// # Errors
    ///
    /// 接不了，例如打开的文件太多了。
    pub async fn accept(&mut self) -> io::Result<Connection> {
        self.socket.accept().await.map(Connection::new)
    }

    /// 套接字在哪。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 拿着的那把锁的文件（施工 TMP-2）：核心照它定时看数据根还在不在。
    pub fn held(&self) -> Held {
        self.held.clone()
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        sys::remove(&self.path);
        // 锁这时还拿着（拿走了的也还在拿走它的那一方手里）：`run/socket` 只会是自己写的。
        if let Err(error) = files::forget_location(&self.root) {
            tracing::debug!(target: "miyu::ipc", error = %error, "socket location not removed");
        }
        // 只删空的：里面有别的东西（不该有）就留着。锁这时还拿着，下一个核心还建不了它。
        if let Some(dir) = &self.own_dir
            && let Err(error) = std::fs::remove_dir(dir)
        {
            tracing::debug!(target: "miyu::ipc", error = %error, "runtime dir not removed");
        }
    }
}

impl fmt::Debug for Listener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Listener")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

/// 连上以后的一个连接：Unix 上是套接字的一头，Windows 上是管道的一头。读写都是异步的字节流，交给
/// 协议端点。
pub struct Connection(sys::Stream);

impl Connection {
    /// 包一层。
    pub(crate) fn new(stream: sys::Stream) -> Connection {
        Connection(stream)
    }
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection").finish_non_exhaustive()
    }
}

impl AsyncRead for Connection {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_read(cx, buf)
    }
}

impl AsyncWrite for Connection {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().0).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_shutdown(cx)
    }
}
