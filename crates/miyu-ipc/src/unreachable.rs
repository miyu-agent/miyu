//! 连不上的核心还在不在（施工 CS-3）：头照 `run/socket` 连不上、读不到时，原来一律当核心没在跑。核心跑着时 `run/` 被删了
//! （人手删的、清理脚本删的；审计 11-03 在 Windows 虚拟机上碰到），它照旧拿着锁、在原处听，只是令牌、位置都没了：`miyu core stop`
//! 说它没在跑、退 0，核心一直开着。
//!
//! 说「没在跑」之前再看两样：这个数据根的锁被拿着没有，照这台机器算出来的那个位置（[`crate::Dirs`]）有没有自己的核心在听。都没有的
//! 才算没在跑。套接字本来就放在 `run/` 里的（macOS，Linux 上没设 `$XDG_RUNTIME_DIR` 的）跟着删了，外面找不到它：它自己定时看到
//! 锁文件没了会走（施工 TMP-2）。

use std::io;

use miyu_store::root::DataRoot;

use crate::error::ConnectError;
use crate::{Dirs, lock, place, sys};

/// 照 `run/socket` 连不上的时候，这个数据根上是不是还有一个核心在跑：锁被拿着，或者算出来的位置上有自己的核心在听。看的时候
/// 连上一下就断开，核心那边记一次连上、断开。
///
/// # Errors
///
/// 打不开锁文件（没有的除外）、拿锁出错；连那个位置时读写出错。
pub async fn running_unreachable(root: &DataRoot) -> io::Result<bool> {
    if lock::held(root)? {
        return Ok(true);
    }
    // 哪里都放不下的，核心也起不来。
    let Ok(path) = place::locate(root, &Dirs::current()) else {
        return Ok(false);
    };
    match sys::connect(&path).await {
        Ok(_) => Ok(true),
        // 别人的：不是这个数据根的核心。
        Err(ConnectError::NotRunning | ConnectError::NotPrivate(_)) => Ok(false),
        Err(ConnectError::Io(error)) => Err(error),
    }
}

#[cfg(test)]
mod tests;
