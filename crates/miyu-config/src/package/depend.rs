//! 依赖的一项（施工 PK-2，`docs/blueprint/packages.md`「依赖」第 1 条）：`[depends]`、`[recommends]` 的 `packages` 里写的一个，
//! 只有编号（`onebot`），或者编号带版本下限（`onebot>=1.2.0`）。只认 `>=` 这一种；下限、装着的版本都照 semver 2.0.0 读、照它的
//! 先后比（预发布的比正式的低，构建信息不算）。读索引也照它（`miyu-source` 的 `index.rs`）：别人的软件源写的不全信。
//!
//! 照 pacman：协议、索引上照写的原样给（`onebot>=1.2.0`），缺的、不够的也照原样报。

use std::cmp::Ordering;
use std::fmt;

use serde::{Serialize, Serializer};

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

/// semver 2.0.0 的一个版本：主、次、修三段数字，可选的预发布（`-` 后面，点分开），构建信息（`+` 后面）只查写法、不比。
#[derive(Debug, PartialEq, Eq)]
struct Semver<'a> {
    /// 主、次、修。
    core: [u64; 3],
    /// 预发布的几段；正式版是空的。
    pre: Vec<&'a str>,
}

impl<'a> Semver<'a> {
    /// 照 semver 2.0.0 的写法读：数字不许有前导的 0（`0` 本身可以），每一段只有字母、数字、`-`，不空；别的读不成。
    fn parse(text: &'a str) -> Option<Semver<'a>> {
        let (text, build) = match text.split_once('+') {
            Some((text, build)) => (text, Some(build)),
            None => (text, None),
        };
        if build.is_some_and(|build| !build.split('.').all(identifier)) {
            return None;
        }
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) => (core, pre.split('.').collect::<Vec<_>>()),
            None => (text, Vec::new()),
        };
        let numbered =
            |part: &&str| !part.bytes().all(|byte| byte.is_ascii_digit()) || number(part);
        if !pre.iter().all(|part| identifier(part) && numbered(part)) {
            return None;
        }
        let mut parts = core.split('.');
        let mut numbers = [0; 3];
        for slot in &mut numbers {
            let part = parts.next().filter(|part| number(part))?;
            *slot = part.parse().ok()?;
        }
        parts
            .next()
            .is_none()
            .then_some(Semver { core: numbers, pre })
    }
}

impl Ord for Semver<'_> {
    /// semver 2.0.0 第 11 条：先比三段数字；一样的，带预发布的比正式的低；都带的一段一段比，数字的照大小、比字母的低，字母的照
    /// ASCII，前面都一样的段少的低。
    fn cmp(&self, other: &Self) -> Ordering {
        self.core
            .cmp(&other.core)
            .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => {
                    let pairs = self.pre.iter().zip(&other.pre);
                    pairs
                        .map(|(mine, theirs)| segment(mine, theirs))
                        .find(|order| order.is_ne())
                        .unwrap_or_else(|| self.pre.len().cmp(&other.pre.len()))
                }
            })
    }
}

impl PartialOrd for Semver<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// 预发布的一段和另一段比。
fn segment(mine: &str, theirs: &str) -> Ordering {
    match (mine.parse::<u64>(), theirs.parse::<u64>()) {
        (Ok(mine), Ok(theirs)) => mine.cmp(&theirs),
        (Ok(_), Err(_)) => Ordering::Less,
        (Err(_), Ok(_)) => Ordering::Greater,
        (Err(_), Err(_)) => mine.cmp(theirs),
    }
}

/// 一段：字母、数字、`-`，不空。
fn identifier(part: &str) -> bool {
    !part.is_empty()
        && part
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

/// 一个数：全是数字、不空、没有前导的 0，读得进 `u64`。
fn number(part: &str) -> bool {
    !part.is_empty()
        && part.bytes().all(|byte| byte.is_ascii_digit())
        && (part == "0" || !part.starts_with('0'))
        && part.parse::<u64>().is_ok()
}

#[cfg(test)]
mod tests;
