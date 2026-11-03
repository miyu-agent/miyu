//! 头自己的设置 `prefs.json`（施工 HP-1，`docs/blueprint/protocol.md`「头自己的设置」）：一个界面包一份，放在它的状态目录
//! （`DataRoot::package_state`）里，卸包时随状态目录一起删。整份就是一个 JSON 对象，键、值由头自己定，这里不认。读照配置文件的
//! 规矩（没有的是空的、1 MiB、UTF-8、版本），写照只给自己看的 JSON 小文件（`private_json`：临时文件、替换前再读一次、Unix 上 0600）。

#[cfg(test)]
mod tests;

use std::path::Path;

use serde_json::{Map, Value};

use crate::config_file::{self, ReadError, WriteError};
use crate::private_json;

/// 文件名：在包的状态目录里。
pub const FILE: &str = "prefs.json";

/// 读到的。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Read {
    /// 整份。没有这个文件的、不是一个 JSON 对象的是空的。
    pub values: Map<String, Value>,
    /// 整份字节的 SHA-256，写回时交给 [`write()`]；没有这个文件的是空的。
    pub version: Option<String>,
    /// 不是一个 JSON 对象（手改坏了）：解析的原话。这时 `values` 是空的，照 `version` 写回就盖掉它。
    pub bad: Option<String>,
}

/// 读 `path`。没有这个文件的是空的；是 JSON 但不是一个对象的也当空的，原话放在 [`Read::bad`]，由调用的一方记下。
///
/// # Errors
///
/// 读不了、超过 1 MiB、不是 UTF-8。
pub fn read(path: &Path) -> Result<Read, ReadError> {
    let Some(text) = config_file::read(path)? else {
        return Ok(Read::default());
    };
    let (values, bad) = match serde_json::from_str(&text.text) {
        Ok(values) => (values, None),
        Err(error) => (Map::new(), Some(error.to_string())),
    };
    Ok(Read {
        values,
        version: Some(text.version),
        bad,
    })
}

/// 把 `values` 整份写进 `path`：一行、末尾一个换行，Unix 上 0600；目录没有的建上。`read` 是上一次读到的版本（那时还没有的是
/// 空的）：对不上的不写。
///
/// # Errors
///
/// 这一瞬间有人手改了（[`WriteError::Changed`]）；写不进。都是什么都没变。
pub fn write(
    path: &Path,
    values: &Map<String, Value>,
    read: Option<&str>,
) -> Result<(), WriteError> {
    private_json::write(path, values, read)
}
