//! 清单写的最低要哪一版 Miyu（施工 X-31，`docs/blueprint/packages.md`「格式」的 `[package] miyu`）：用了新加的格、新的方法的包
//! 写上它，旧核心读到先比它，比不过的说清要更新 Miyu，不再报不认识的键。清单照旧严格：写错的键还是报错。
//!
//! 正在跑的版本由调用的一方交进来（纯逻辑这一层不读编译时的环境，`01-架构.md` 第九节第 3 条）；`0.0.0` 是开发时编的、没发过版，
//! 当最新、不比（主会话审时定）。

use toml_edit::{Document, Item};

use super::reader::{Reader, line_of};
use super::{Code, Problem};

/// 三段数字的版本：主、次、修。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Version([u64; 3]);

impl Version {
    /// 开发时编的那一版：当最新。
    const DEVELOPMENT: Version = Version([0, 0, 0]);

    /// 照 `<主>.<次>.<修>` 认；别的写法（少一段、多一段、带字母、带前导的 `v`）不认。
    fn parse(text: &str) -> Option<Version> {
        let mut parts = text.split('.');
        let mut numbers = [0; 3];
        for number in &mut numbers {
            let part = parts.next()?;
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            *number = part.parse().ok()?;
        }
        parts.next().is_none().then_some(Version(numbers))
    }

    /// 要 `required` 的包，这一版用不用得了。
    fn fits(self, required: Version) -> bool {
        self == Version::DEVELOPMENT || self >= required
    }
}

/// `[package] miyu` 的写法对不对（[`super::read`] 里查）。
pub(super) fn checked(reader: &Reader<'_>, item: &Item) -> Result<(), Problem> {
    item.as_str()
        .and_then(Version::parse)
        .map(|_| ())
        .ok_or_else(|| {
            reader.problem(
                Some(item),
                Code::BadMiyu,
                "package.miyu",
                "package.miyu must be a version like 0.2.0".to_string(),
            )
        })
}

/// 清单 `text` 要的 Miyu 这一版 `running` 给不给得了：给得了的（没写、写的不比它新、它是开发版）交回空的；给不了的报
/// `needs_newer_miyu`，`detail` 是清单写的版本。读不成 TOML、`[package]` 不是表、写法不对的不在这里报，交给 [`super::read`]；
/// `running` 本身不是三段数字的（不会有：它是编出来的版本号）也不比。
pub(super) fn required(text: &str, running: &str) -> Result<(), Problem> {
    let (Ok(document), Some(running)) = (Document::parse(text), Version::parse(running)) else {
        return Ok(());
    };
    let Some(item) = document
        .as_table()
        .get("package")
        .and_then(Item::as_table_like)
        .and_then(|package| package.get("miyu"))
    else {
        return Ok(());
    };
    let Some(wanted) = item.as_str().and_then(Version::parse) else {
        return Ok(());
    };
    if running.fits(wanted) {
        return Ok(());
    }
    let text_of = item.as_str().unwrap_or_default();
    Err(Problem {
        line: item.span().map(|span| line_of(text, span.start)),
        code: Code::NeedsNewerMiyu,
        detail: text_of.to_string(),
        message: format!("this package needs Miyu {text_of} or newer"),
    })
}

#[cfg(test)]
mod tests;
