//! 扩展要常驻的账号 `system/kept.json`（施工 IM-1 下，接入通讯平台设计 06 第 4.6 节第 1 样）：每人一份的扩展经
//! `process.keep` 说替这个人常驻，核心起来时只给记着的账号拉起。扩展自己说的，不是人做的决定，但不能删掉重建（删了的话没起
//! 的那一份没机会再说），所以放 `system/`、不放 `state/`。读写照只给自己看的 JSON 小文件的规矩。

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config_file::WriteError;
use crate::private_json;
pub use crate::private_json::BadFile;

/// 文件名：在 `system/` 里。
pub const FILE: &str = "kept.json";

/// 现在的写法。
const VERSION: u32 = 1;

/// 读好的一份。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kept {
    /// 写法的版本：现在是 1。
    pub version: u32,
    /// 包的编号到要常驻的账号：一个都没有的包不写。
    pub kept: BTreeMap<String, BTreeSet<String>>,
}

impl Default for Kept {
    fn default() -> Kept {
        Kept {
            version: VERSION,
            kept: BTreeMap::new(),
        }
    }
}

impl Kept {
    /// 包 `package` 替 `account` 常驻没有。
    pub fn on(&self, package: &str, account: &str) -> bool {
        self.kept
            .get(package)
            .is_some_and(|accounts| accounts.contains(account))
    }

    /// 记下包 `package` 替 `account` 常驻（`on`）或者不了；一个都没有的包拿掉。
    pub fn set(&mut self, package: &str, account: &str, on: bool) {
        if on {
            self.kept
                .entry(package.to_string())
                .or_default()
                .insert(account.to_string());
        } else if let Some(accounts) = self.kept.get_mut(package) {
            accounts.remove(account);
            if accounts.is_empty() {
                self.kept.remove(package);
            }
        }
    }
}

/// 读到的：常驻的账号和这份文件的版本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read {
    /// 读好的。没有这个文件的是空的。
    pub kept: Kept,
    /// 整份字节的 SHA-256；没有这个文件的是空的。
    pub version: Option<String>,
}

/// 读 `path`。没有这个文件的是空的。
///
/// # Errors
///
/// 读不了、太大、不是 UTF-8；不是这个形状、版本不是 1。
pub fn read(path: &Path) -> Result<Read, BadFile> {
    let (kept, version): (Kept, _) = private_json::read(path)?;
    if kept.version != VERSION {
        return Err(BadFile::Shape(format!("unknown version {}", kept.version)));
    }
    Ok(Read { kept, version })
}

/// 把 `kept` 整份写进 `path`，Unix 上 0600。`read` 是上一次读到的版本（那时还没有、读不成的是空的）。
///
/// # Errors
///
/// 这一瞬间有人手改了（[`WriteError::Changed`]）；写不进。都是什么都没变。
pub fn write(path: &Path, kept: &Kept, read: Option<&str>) -> Result<(), WriteError> {
    private_json::write(path, kept, read)
}
