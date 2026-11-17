//! 人停的核心的记号 `run/stopped`（施工 CS-5，2026-11-17 项目主人定「停了就停」）：人叫停的（`miyu core stop`、头上的
//! 「停止」，都走 `core.stop`）核心停之前留下它；头断线以后自动拉起（[`crate::reconnect_or_start`]）见到它不拉，交回
//! [`crate::StartError::StoppedByUser`]，由人点「启动」、敲命令（[`crate::connect_or_start`]）再拉。核心每次起来拿到锁就删掉它：
//! 不管是谁、怎么拉起来的，起来了就不再是「人停的」。崩了、空闲退出、收到信号停的不留，头照旧自动拉起。
//!
//! 只是一个空文件，在不在就是意思；不同步，断电丢了的当没停过（照旧自动拉起，和以前一样）。

use std::fs;
use std::io;

use miyu_store::root::DataRoot;

/// 记号的名字，在 `run/` 里。
const STOPPED: &str = "stopped";

/// 留下「人停的」记号：核心收到 `core.stop` 时调，那时锁还在它手里。
///
/// # Errors
///
/// 写不进。
pub fn mark_stopped(root: &DataRoot) -> io::Result<()> {
    fs::write(root.run().join(STOPPED), b"")
}

/// 有没有「人停的」记号：读不了的当没有（照旧自动拉起）。
pub(crate) fn stopped_by_user(root: &DataRoot) -> bool {
    root.run().join(STOPPED).is_file()
}

/// 核心起来拿到锁以后删掉记号：没有的不算错。
///
/// # Errors
///
/// 删不掉。
pub(crate) fn clear(root: &DataRoot) -> io::Result<()> {
    match fs::remove_file(root.run().join(STOPPED)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
