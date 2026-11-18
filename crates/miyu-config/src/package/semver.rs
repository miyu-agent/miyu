//! semver 2.0.0 的版本和先后（施工 PK-2 写在 `depend.rs`，施工 VER-1 挪出来）：依赖的下限、清单 `[package] miyu` 要的那一版都
//! 照它读、照它比，预发布的比同号正式的低（`2026.11.0-rc.2.3` < `2026.11.0-rc.2.4` < `2026.11.0`）。

use std::cmp::Ordering;

/// semver 2.0.0 的一个版本：主、次、修三段数字，可选的预发布（`-` 后面，点分开），构建信息（`+` 后面）只查写法、不比。
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Semver<'a> {
    /// 主、次、修。
    pub(super) core: [u64; 3],
    /// 预发布的几段；正式版是空的。
    pub(super) pre: Vec<&'a str>,
}

impl<'a> Semver<'a> {
    /// 照 semver 2.0.0 的写法读：数字不许有前导的 0（`0` 本身可以），每一段只有字母、数字、`-`，不空；别的读不成。
    pub(super) fn parse(text: &'a str) -> Option<Semver<'a>> {
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
