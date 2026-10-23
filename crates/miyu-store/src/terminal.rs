//! 终端会话的指针 `home/<账号>/terminal.json`（施工 8-33，设计 `22-命令行.md` 第七节）：这个账号在终端里说的话进哪个会话。
//! 哪个会话算终端会话、闲置多久换新的，由端点照它和会话列表的索引定，这里只读写这一格。读、写照 `projects.json`（只给自己看的
//! JSON 小文件：0600、照读到的版本写）。
//!
//! 按终端分开（施工 T-22，2026-10-23 项目主人定：不同终端不同会话）：头报一个终端的键，各记一个指针（[`Pointers::terminals`]），
//! 不报键的照旧用顶上那一个。键各记最后一次用到的时刻，写的时候顺手清掉久没用的（[`Pointers::prune`]），不另起定时器。

use std::collections::BTreeMap;
use std::path::Path;

use miyu_kernel::time::Timestamp;
use serde::{Deserialize, Serialize};

use crate::config_file::WriteError;
use crate::private_json;
pub use crate::private_json::BadFile;

/// 文件名：在账号的家目录里。
pub const FILE: &str = "terminal.json";

/// 键最多记几个（施工 T-22）：多了丢最久没用的。
pub const MOST_TERMINALS: usize = 64;

/// 键多久没用就清掉（施工 T-22）：7 天。
pub const FORGET_AFTER_MS: i64 = 7 * 24 * 60 * 60 * 1000;

/// 整份文件：不报键的那一个指针在顶上（以前的写法原样认），按终端的键各一个在 `terminals` 里（施工 T-22）。不认识的格不拦。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pointers {
    /// 不报键的那一个（施工 8-33 起的那一份）。
    #[serde(flatten)]
    pub main: Pointer,
    /// 按终端的键各一个（施工 T-22）：键是头生成的，核心只当不透明的字。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub terminals: BTreeMap<String, Keyed>,
}

/// 一个终端的指针，带最后一次用到的时刻（只拿来清久没用的，闲置照 [`Pointer::at`] 和会话自己的动静算）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keyed {
    /// 指针本身：同顶上那一个的写法。
    #[serde(flatten)]
    pub pointer: Pointer,
    /// 最后一次问到、写到它的时刻。
    pub used: Timestamp,
}

impl Pointers {
    /// 清掉 `now` 之前超过 [`FORGET_AFTER_MS`] 没用的键；还多于 [`MOST_TERMINALS`] 的丢最久没用的。
    pub fn prune(&mut self, now: Timestamp) {
        let cutoff = now.unix_millis() - FORGET_AFTER_MS;
        self.terminals
            .retain(|_, keyed| keyed.used.unix_millis() >= cutoff);
        while self.terminals.len() > MOST_TERMINALS {
            let oldest = self
                .terminals
                .iter()
                .min_by_key(|(_, keyed)| keyed.used.unix_millis())
                .map(|(key, _)| key.clone());
            match oldest {
                Some(key) => drop(self.terminals.remove(&key)),
                None => break,
            }
        }
    }
}

/// 一个指针：没有文件的、还没指过的没有会话。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pointer {
    /// 终端会话的编号。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// 指过去的时刻（施工 8-33 三补，2026-10-21 项目主人定：`/sessions` 主动切过去的不该再被闲置规则换掉）：算这个终端会话的一次
    /// 动静，闲置从这一刻起算。以前写的没有，照会话自己的最后一次动静。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Timestamp>,
}

/// 读 `path`：交回整份和读到的版本（写回时交给 [`write()`]）。没有这个文件的是空的、版本是空的。
///
/// # Errors
///
/// 读不了、太大、不是 UTF-8；不是这个形状。
pub fn read(path: &Path) -> Result<(Pointers, Option<String>), BadFile> {
    private_json::read(path)
}

/// 把 `pointers` 整份写进 `path`，Unix 上 0600。`read` 是上一次读到的版本（那时还没有的是空的）。
///
/// # Errors
///
/// 这一瞬间有人手改了（[`WriteError::Changed`]）；写不进。都是什么都没变。
pub fn write(path: &Path, pointers: &Pointers, read: Option<&str>) -> Result<(), WriteError> {
    private_json::write(path, pointers, read)
}

#[cfg(test)]
mod tests;
