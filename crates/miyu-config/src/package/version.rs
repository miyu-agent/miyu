//! 清单写的最低要哪一版 Miyu（施工 X-31，`internal/blueprint/packages.md`「格式」的 `[package] miyu`）：用了新加的格、新的方法的包
//! 写上它，旧核心读到先比它，比不过的说清要更新 Miyu，不再报不认识的键。清单照旧严格：写错的键还是报错。
//!
//! 正在跑的版本由调用的一方交进来（纯逻辑这一层不读编译时的环境，`01-架构.md` 第九节第 3 条）；`0.0.0` 是开发时编的、没发过版，
//! 当最新、不比（主会话审时定）。版本照 semver 读、照它的先后比（施工 VER-1，`semver.rs`）：候选版（`2026.11.0-rc.2.3`）也比，
//! 原来只认三段数字，候选版的核心读到候选版的要求两头都读不成、不比，报成了不认识的表。

use toml_edit::{Document, Item};

use super::reader::{Reader, line_of};
use super::semver::Semver;
use super::{Code, Problem};

/// 开发时编的那一版（`0.0.0`，不带预发布）：当最新。
fn development(version: &Semver<'_>) -> bool {
    version.core == [0, 0, 0] && version.pre.is_empty()
}

/// 正在跑的 `running` 用不用得了要 `required` 的包：开发版都用得了，别的照 semver 的先后（施工 VER-1：候选版也比，
/// `2026.11.0-rc.2.3` 用不了要 `2026.11.0-rc.2.4` 的，也用不了要 `2026.11.0` 的）。
fn fits(running: &Semver<'_>, required: &Semver<'_>) -> bool {
    development(running) || running >= required
}

/// `[package] miyu` 的写法对不对（[`super::read`] 里查）。
pub(super) fn checked(reader: &Reader<'_>, item: &Item) -> Result<(), Problem> {
    item.as_str()
        .and_then(Semver::parse)
        .map(|_| ())
        .ok_or_else(|| {
            reader.problem(
                Some(item),
                Code::BadMiyu,
                "package.miyu",
                "package.miyu must be a version like 2026.11.0".to_string(),
            )
        })
}

/// 清单 `text` 要的 Miyu 这一版 `running` 给不给得了：给得了的（没写、写的不比它新、它是开发版）交回空的；给不了的报
/// `needs_newer_miyu`，`detail` 是清单写的版本。读不成 TOML、`[package]` 不是表、写法不对的不在这里报，交给 [`super::read`]；
/// `running` 本身读不成的（不会有：它是编出来的版本号，同 `miyu --version`）也不比。
pub(super) fn required(text: &str, running: &str) -> Result<(), Problem> {
    let (Ok(document), Some(running)) = (Document::parse(text), Semver::parse(running)) else {
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
    let Some(wanted) = item.as_str().and_then(Semver::parse) else {
        return Ok(());
    };
    if fits(&running, &wanted) {
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

/// 读不成的这一处可能是这一版 Miyu 太旧（施工 VER-1）：报的是不认识的表、键，清单又写了 `[package] miyu`（比得过的、开发版不比
/// 的都算：新加的表、键旧核心不认识）。调的一方在报错后面补一句「这个包可能要更新的 Miyu」。
pub fn maybe_newer(text: &str, problem: &Problem) -> bool {
    matches!(problem.code, Code::UnknownTable | Code::UnknownKey)
        && Document::parse(text).is_ok_and(|document| {
            document
                .as_table()
                .get("package")
                .and_then(Item::as_table_like)
                .and_then(|package| package.get("miyu"))
                .is_some_and(|item| item.as_str().is_some())
        })
}

#[cfg(test)]
mod tests;
