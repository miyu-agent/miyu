//! 单实例锁（`docs/designs/07-存储.md` 第十节）：一个数据根上只跑一个核心。
//!
//! 锁在 `run/core.lock` 上，用标准库的文件锁，三个平台一样。一直拿到核心退出：进程没了，操作系统
//! 替它放开，崩了也不会留下一把死锁。同一个进程里打开两次也是两把，第二把拿不到。
//!
//! 数据根被删了（测试删临时目录、人手删的）拿着锁的核心不知道：锁还在它打开的那个文件上，路径上已经没有了，下一个核心
//! 在新建的数据根上照样拿得到一把。核心照 [`Held::here`] 定时看一眼，不在了就走（施工 TMP-2）。

use std::fs::{File, OpenOptions, TryLockError};
use std::path::PathBuf;

use miyu_store::root::DataRoot;

use crate::error::OpenError;

/// 锁文件的名字，在 `run/` 里。
const FILE: &str = "core.lock";

/// 拿着的单实例锁：丢掉它就放开。
#[derive(Debug)]
pub struct Lock {
    /// 锁在这个打开的文件上：文件关了，锁就放开。
    _file: File,
    /// 锁文件在哪、是哪一个（施工 TMP-2）。
    held: Held,
}

/// 拿着的那把锁的文件在哪、是哪一个（施工 TMP-2）：看数据根里的锁文件还是不是它。
#[derive(Debug, Clone)]
pub struct Held {
    path: PathBuf,
    /// Unix 上是设备号和 inode；Windows 上开着的文件删不掉、改不了名，只看在不在，没有。
    id: Option<(u64, u64)>,
}

impl Lock {
    /// 拿锁，不等：拿不到就是已经有一个核心在跑。
    ///
    /// # Errors
    ///
    /// 拿不到；打不开锁文件。
    pub fn acquire(root: &DataRoot) -> Result<Lock, OpenError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.run().join(FILE))?;
        match file.try_lock() {
            Ok(()) => {
                let held = Held {
                    id: identity(&file.metadata()?),
                    path: root.run().join(FILE),
                };
                Ok(Lock { _file: file, held })
            }
            Err(TryLockError::WouldBlock) => Err(OpenError::Running),
            Err(TryLockError::Error(error)) => Err(OpenError::Io(error)),
        }
    }

    /// 锁文件在哪、是哪一个：交给定时看的那一边。
    pub fn held(&self) -> Held {
        self.held.clone()
    }
}

/// 等这个数据根上的核心放开锁（施工 CL-4，`miyu core stop`）：阻塞地拿一次锁，拿到就放。不轮询，进程一没操作系统就放锁、
/// 这里就醒。锁文件没有的（从没起过核心）当已经放了。在阻塞线程里调。
///
/// # Errors
///
/// 打不开锁文件、拿锁出错。
pub fn wait_released(root: &DataRoot) -> std::io::Result<()> {
    let file = match OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.run().join(FILE))
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    // 拿到就放：这时要起的新核心照旧拿得到。
    file.lock()?;
    file.unlock()
}

impl Held {
    /// 数据根里的锁文件还是拿着的这一个：路径上还有，而且是同一个文件（Unix 上照设备号和 inode）。数据根删了的、删了又建了
    /// 一个新的，不是。读不出来的当不是。碰一次磁盘。
    pub fn here(&self) -> bool {
        match std::fs::metadata(&self.path) {
            Ok(now) => identity(&now) == self.id,
            Err(_) => false,
        }
    }
}

/// 文件是哪一个：Unix 上是设备号和 inode，别的平台没有。
#[cfg(unix)]
#[expect(clippy::unnecessary_wraps, reason = "别的平台没有，交回的样子要一样")]
fn identity(metadata: &std::fs::Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    Some((metadata.dev(), metadata.ino()))
}

/// 文件是哪一个：Windows 上没有。
#[cfg(not(unix))]
fn identity(_: &std::fs::Metadata) -> Option<(u64, u64)> {
    None
}

#[cfg(test)]
mod tests;
