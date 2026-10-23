//! 组表 `system/groups.json`（施工 Y-12 中下，设计 06 U16、U19 第 4 条）：组名到成员账号编号，人和包同一张表。人的组不带点（现在
//! 只有 [`ADMIN`] 管理员组）；扩展能力的组是 `ext.<能力>`（施工 Y-5 下上）。机主恒是管理员，不写进表也算（判法在端点）。读写照
//! 只给自己看的 JSON 小文件的规矩：带版本、Unix 上 0600、没有这个文件的是空的。

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config_file::WriteError;
use crate::private_json;
pub use crate::private_json::BadFile;

/// 文件名：在 `system/` 里。
pub const FILE: &str = "groups.json";

/// 管理员组。
pub const ADMIN: &str = "admin";

/// 现在的写法。
const VERSION: u32 = 1;

/// 读好的一份。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Groups {
    /// 写法的版本：现在是 1。
    pub version: u32,
    /// 组名到成员的账号编号，照编号排、不重；空了的组不写。
    pub groups: BTreeMap<String, Vec<String>>,
}

impl Default for Groups {
    fn default() -> Groups {
        Groups {
            version: VERSION,
            groups: BTreeMap::new(),
        }
    }
}

impl Groups {
    /// 组 `group` 的成员，照编号排；没有这个组的是空的。
    #[must_use]
    pub fn members(&self, group: &str) -> &[String] {
        self.groups.get(group).map_or(&[], Vec::as_slice)
    }

    /// `account` 在不在组 `group` 里。
    #[must_use]
    pub fn has(&self, group: &str, account: &str) -> bool {
        self.members(group).iter().any(|one| one == account)
    }

    /// 把 `account` 加进组 `group`：交回是不是新加的（本来就在的不动）。
    pub fn add(&mut self, group: &str, account: &str) -> bool {
        let members = self.groups.entry(group.to_string()).or_default();
        match members.binary_search_by(|one| one.as_str().cmp(account)) {
            Ok(_) => false,
            Err(at) => {
                members.insert(at, account.to_string());
                true
            }
        }
    }

    /// 把 `account` 移出组 `group`：交回是不是本来在。组空了整组拿掉。
    pub fn remove(&mut self, group: &str, account: &str) -> bool {
        let Some(members) = self.groups.get_mut(group) else {
            return false;
        };
        let before = members.len();
        members.retain(|one| one != account);
        let removed = members.len() != before;
        if members.is_empty() {
            self.groups.remove(group);
        }
        removed
    }
}

/// 读到的：组表和这份文件的版本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read {
    /// 读好的。没有这个文件的是空的。
    pub groups: Groups,
    /// 整份字节的 SHA-256；没有这个文件的是空的。
    pub version: Option<String>,
}

/// 读 `path`。没有这个文件的是空的。
///
/// # Errors
///
/// 读不了、太大、不是 UTF-8；不是这个形状、版本不是 1。
pub fn read(path: &Path) -> Result<Read, BadFile> {
    let (groups, version): (Groups, _) = private_json::read(path)?;
    if groups.version != VERSION {
        return Err(BadFile::Shape(format!(
            "unknown version {}",
            groups.version
        )));
    }
    Ok(Read { groups, version })
}

/// 把 `groups` 整份写进 `path`，Unix 上 0600。`read` 是上一次读到的版本（那时还没有、读不成的是空的）。
///
/// # Errors
///
/// 这一瞬间有人手改了（[`WriteError::Changed`]）；写不进。都是什么都没变。
pub fn write(path: &Path, groups: &Groups, read: Option<&str>) -> Result<(), WriteError> {
    private_json::write(path, groups, read)
}
