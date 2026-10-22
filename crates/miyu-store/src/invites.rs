//! 邀请码 `system/invites.json`（施工 Y-1 上，设计 06 第六节、U15）：管理员发的一次性码，凭它注册成员。只存码的 SHA-256，
//! 原文只在发的那一刻交给管理员一次。一行写编号（列出来、撤销时用）、谁发的、发的时刻、过期的时刻，用掉了记谁、什么时候。
//!
//! 每次写的时候删掉过期没用的、用掉超过 30 天的；最多 64 行，多了删最早发的。

use std::path::Path;

use serde::{Deserialize, Serialize};

use miyu_kernel::time::Timestamp;

use crate::config_file::WriteError;
use crate::private_json;
pub use crate::private_json::BadFile;

/// 文件名：在 `system/` 里。
pub const FILE: &str = "invites.json";

/// 现在的写法。
const VERSION: u32 = 1;

/// 最多记几个。
const MOST: usize = 64;

/// 用掉的留多久（毫秒）：30 天。
const KEEP_USED_MS: i64 = 30 * 86_400_000;

/// 读好的一份。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invites {
    /// 写法的版本：现在是 1。
    pub version: u32,
    /// 一个邀请码一行，照发的先后。
    pub invites: Vec<Invite>,
}

impl Default for Invites {
    fn default() -> Invites {
        Invites {
            version: VERSION,
            invites: Vec::new(),
        }
    }
}

/// 一个邀请码。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invite {
    /// 编号：8 位小写十六进制。
    pub id: String,
    /// 码的 SHA-256（[`crate::logins::digest`]）。
    pub hash: String,
    /// 谁发的（账号编号）。
    pub by: String,
    /// 什么时候发的。
    pub created: Timestamp,
    /// 什么时候过期。
    pub expires: Timestamp,
    /// 用掉了：谁、什么时候。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used: Option<Used>,
}

/// 谁、什么时候用掉的。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Used {
    /// 用它注册的账号。
    pub account: String,
    /// 什么时候。
    pub at: Timestamp,
}

impl Invites {
    /// 加上 `invite`：先删掉旧的（[`Invites::prune`]）；多过 64 个的删最早发的。
    pub fn add(&mut self, invite: Invite, now: Timestamp) {
        self.prune(now);
        self.invites.push(invite);
        while self.invites.len() > MOST {
            self.invites.remove(0);
        }
    }

    /// 删掉 `now` 时已经过期没用的、用掉超过 30 天的。
    pub fn prune(&mut self, now: Timestamp) {
        let cutoff = now.unix_millis() - KEEP_USED_MS;
        self.invites.retain(|each| match &each.used {
            Some(used) => used.at.unix_millis() > cutoff,
            None => each.expires > now,
        });
    }

    /// 哈希是 `hash` 的码在 `now` 时能不能用：有、没过期、没用掉。交回它的编号。
    pub fn usable(&self, hash: &str, now: Timestamp) -> Option<&str> {
        self.invites
            .iter()
            .find(|each| each.hash == hash && each.used.is_none() && each.expires > now)
            .map(|each| each.id.as_str())
    }

    /// 用掉哈希是 `hash` 的码：能用的记下 `account`、`now`，交回它的编号。
    pub fn take(&mut self, hash: &str, account: &str, now: Timestamp) -> Option<String> {
        let invite = self
            .invites
            .iter_mut()
            .find(|each| each.hash == hash && each.used.is_none() && each.expires > now)?;
        invite.used = Some(Used {
            account: account.to_string(),
            at: now,
        });
        Some(invite.id.clone())
    }

    /// 撤掉编号是 `id`、还没用掉的：有的交回真。
    pub fn revoke(&mut self, id: &str) -> bool {
        let before = self.invites.len();
        self.invites
            .retain(|each| !(each.id == id && each.used.is_none()));
        self.invites.len() < before
    }
}

/// 读到的一份和它的版本（写回时交给 [`write()`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read {
    /// 读好的。没有这个文件的是空的。
    pub invites: Invites,
    /// 整份字节的 SHA-256；没有这个文件的是空的。
    pub version: Option<String>,
}

/// 读 `path`。没有这个文件的是空的。
///
/// # Errors
///
/// 读不了、太大、不是 UTF-8；不是这个形状、版本不是 1。
pub fn read(path: &Path) -> Result<Read, BadFile> {
    let (invites, version): (Invites, _) = private_json::read(path)?;
    if invites.version != VERSION {
        return Err(BadFile::Shape(format!(
            "unknown version {}",
            invites.version
        )));
    }
    Ok(Read { invites, version })
}

/// 把 `invites` 整份写进 `path`，Unix 上 0600。`read` 是上一次读到的版本（那时还没有的是空的）。
///
/// # Errors
///
/// 这一瞬间有人手改了（[`WriteError::Changed`]）；写不进。都是什么都没变。
pub fn write(path: &Path, invites: &Invites, read: Option<&str>) -> Result<(), WriteError> {
    private_json::write(path, invites, read)
}

#[cfg(test)]
mod tests;
