//! `[package]` 的作者、上游和许可证（施工 F-12，项目主人 2026-10-19 定：每个包都写，软件包信息页露出来）：`author` 是一段字
//! （人名、组织名，可以带邮箱），`upstream` 是这个包的源码仓库，`https://` 开头的网址，`license` 是许可证（施工 PV-3，项目主人
//! 2026-11-12 定加一格、可以不写）：建议写 SPDX 表达式（`MIT`、`MIT OR Apache-2.0`），只查是一行字、不超长，不查是不是合法的
//! SPDX。都可以不写；写错的报 `bad_author`、`bad_upstream`、`bad_license`。

use toml_edit::TableLike;

use super::reader::Reader;
use super::{Code, Problem};

/// 作者至多几个字。
const AUTHOR_MAX: usize = 100;

/// 网址至多几个字。
const UPSTREAM_MAX: usize = 300;

/// 许可证至多几个字：SPDX 的表达式一般十来个字，几个许可证拼起来的也远不到一百。
const LICENSE_MAX: usize = 100;

/// 读成的作者、上游。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Credits {
    pub(super) author: Option<String>,
    pub(super) upstream: Option<String>,
    pub(super) license: Option<String>,
}

/// 读 `[package]` 里的 `author`、`upstream`、`license`。
pub(super) fn read(reader: &Reader<'_>, table: &dyn TableLike) -> Result<Credits, Problem> {
    let author = line(reader, table, ("author", AUTHOR_MAX), Code::BadAuthor)?;
    let upstream = match table.get("upstream") {
        None => None,
        Some(item) => {
            let text = item.as_str().unwrap_or_default();
            let fine = text.len() > "https://".len()
                && text.starts_with("https://")
                && text.chars().count() <= UPSTREAM_MAX
                && !text.chars().any(|c| c.is_whitespace() || c.is_control());
            if !fine {
                return Err(reader.problem(
                    Some(item),
                    Code::BadUpstream,
                    "package.upstream",
                    format!("package.upstream must be an https:// address of at most {UPSTREAM_MAX} characters"),
                ));
            }
            Some(text.to_string())
        }
    };
    let license = line(reader, table, ("license", LICENSE_MAX), Code::BadLicense)?;
    Ok(Credits {
        author,
        upstream,
        license,
    })
}

/// `[package]` 里一格一行的字 `key`：去掉前后空白以后 1 到 `most` 个字、不带控制字符（换行也算），没写的没有，写错的报 `code`。
fn line(
    reader: &Reader<'_>,
    table: &dyn TableLike,
    (key, most): (&str, usize),
    code: Code,
) -> Result<Option<String>, Problem> {
    let Some(item) = table.get(key) else {
        return Ok(None);
    };
    let text = item.as_str().map(str::trim).unwrap_or_default();
    let fine =
        !text.is_empty() && text.chars().count() <= most && !text.chars().any(char::is_control);
    if !fine {
        return Err(reader.problem(
            Some(item),
            code,
            &format!("package.{key}"),
            format!("package.{key} must be text of 1 to {most} characters, on one line"),
        ));
    }
    Ok(Some(text.to_string()))
}

#[cfg(test)]
mod tests;
