//! 依赖的一项（施工 PK-2，`internal/blueprint/packages.md`「依赖」第 1 条）：`[depends]`、`[recommends]` 的 `packages` 里写的一个，
//! 只有编号（`onebot`），或者编号带版本下限（`onebot>=1.2.0`）。只认 `>=` 这一种；下限、装着的版本都照 semver 2.0.0 读、照它的
//! 先后比（预发布的比正式的低，构建信息不算）。读索引也照它（`miyu-source` 的 `index.rs`）：别人的软件源写的不全信。
//!
//! 照 pacman：协议、索引上照写的原样给（`onebot>=1.2.0`），缺的、不够的也照原样报。

use std::cmp::Ordering;
use std::fmt;

use serde::{Serialize, Serializer};

use super::semver::Semver;
use crate::secret::valid_name;

/// 依赖、推荐的一个包。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    /// 包的编号。
    pub id: String,
    /// 至少要哪一版（`>=` 后面的），照写的原样；没写的没有，装着就行。
    pub at_least: Option<String>,
}

impl Dependency {
    /// 照写的读：`<编号>` 或 `<编号>>=<版本>`，编号照包编号的写法，版本照 semver（`1.2.0`、`1.2.0-rc.1`），前后、中间都不许有
    /// 空格。别的写法（`=`、`<`、`~`、两段的 `1.2`、带 `v` 的）读不成。
    pub fn parse(text: &str) -> Option<Dependency> {
        let (id, at_least) = match text.split_once(">=") {
            Some((id, version)) => (id, Some(version)),
            None => (text, None),
        };
        if !valid_name(id) || at_least.is_some_and(|version| Semver::parse(version).is_none()) {
            return None;
        }
        Some(Dependency {
            id: id.to_string(),
            at_least: at_least.map(str::to_string),
        })
    }

    /// 装着的那一份（清单写的版本 `version`）够不够：没下限的装着就够；有下限的，版本没写、不是 semver 的不够，比下限低的不够。
    pub fn fits(&self, version: Option<&str>) -> bool {
        let Some(at_least) = &self.at_least else {
            return true;
        };
        match (version.and_then(Semver::parse), Semver::parse(at_least)) {
            (Some(have), Some(want)) => have.cmp(&want) != Ordering::Less,
            _ => false,
        }
    }
}

impl fmt::Display for Dependency {
    /// 照写的原样：`onebot`、`onebot>=1.2.0`。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.at_least {
            Some(version) => write!(f, "{}>={version}", self.id),
            None => f.write_str(&self.id),
        }
    }
}

impl Serialize for Dependency {
    /// 协议上是一个字，照写的原样（同 [`fmt::Display`]）。
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(test)]
mod tests;
