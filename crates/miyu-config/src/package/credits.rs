//! `[package]` 的作者和上游（施工 F-12，项目主人 2026-10-19 定：每个包都写，软件包信息页露出来）：`author` 是一段字（人名、
//! 组织名，可以带邮箱），`upstream` 是这个包的源码仓库，`https://` 开头的网址。都可以不写；写错的报 `bad_author`、
//! `bad_upstream`。

use toml_edit::TableLike;

use super::reader::Reader;
use super::{Code, Problem};

/// 作者至多几个字。
const AUTHOR_MAX: usize = 100;

/// 网址至多几个字。
const UPSTREAM_MAX: usize = 300;

/// 读成的作者、上游。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Credits {
    pub(super) author: Option<String>,
    pub(super) upstream: Option<String>,
}

/// 读 `[package]` 里的 `author`、`upstream`。
pub(super) fn read(reader: &Reader<'_>, table: &dyn TableLike) -> Result<Credits, Problem> {
    let author = match table.get("author") {
        None => None,
        Some(item) => {
            let text = item.as_str().map(str::trim).unwrap_or_default();
            let fine = !text.is_empty()
                && text.chars().count() <= AUTHOR_MAX
                && !text.chars().any(char::is_control);
            if !fine {
                return Err(reader.problem(
                    Some(item),
                    Code::BadAuthor,
                    "package.author",
                    format!(
                        "package.author must be text of 1 to {AUTHOR_MAX} characters, on one line"
                    ),
                ));
            }
            Some(text.to_string())
        }
    };
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
    Ok(Credits { author, upstream })
}

#[cfg(test)]
mod tests;
