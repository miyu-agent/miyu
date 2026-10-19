//! 终端会话的指针 `home/<账号>/terminal.json`（施工 8-33，设计 `22-命令行.md` 第七节）：这个账号在终端里说的话进哪个会话。
//! 哪个会话算终端会话、闲置多久换新的，由端点照它和会话列表的索引定，这里只读写这一格。读、写照 `projects.json`（只给自己看的
//! JSON 小文件：0600、照读到的版本写）。

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config_file::WriteError;
use crate::private_json;
pub use crate::private_json::BadFile;

/// 文件名：在账号的家目录里。
pub const FILE: &str = "terminal.json";

/// 读好的一份：没有文件的、还没指过的没有会话。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pointer {
    /// 终端会话的编号。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

/// 读 `path`：交回指针和读到的版本（写回时交给 [`write()`]）。没有这个文件的是空的、版本是空的。
///
/// # Errors
///
/// 读不了、太大、不是 UTF-8；不是这个形状。
pub fn read(path: &Path) -> Result<(Pointer, Option<String>), BadFile> {
    private_json::read(path)
}

/// 把 `pointer` 整份写进 `path`，Unix 上 0600。`read` 是上一次读到的版本（那时还没有的是空的）。
///
/// # Errors
///
/// 这一瞬间有人手改了（[`WriteError::Changed`]）；写不进。都是什么都没变。
pub fn write(path: &Path, pointer: &Pointer, read: Option<&str>) -> Result<(), WriteError> {
    private_json::write(path, pointer, read)
}
