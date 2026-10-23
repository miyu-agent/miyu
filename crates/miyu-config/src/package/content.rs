//! `[content.<包>]`（施工 X-22，设计 `34-表情包.md` 第三节「内容包」、`docs/blueprint/packages.md`「给别的包带的内容」）：这个包
//! 给另一个包带了一个目录的东西（表情包的图、人设卡这些），`dir` 是包目录里的相对目录，写法同 `[page] dir`。被带的那个包的扩展经
//! `content.list`、`content.read` 读，核心不认里面是什么。一张表一个包，照包的编号排；只有 `bundle`、扩展包能写。

use toml_edit::Item;

use super::reader::{Reader, relative_dir};
use super::{Code, Problem};
use crate::secret::valid_name;

/// 给 `target` 那个包带的内容在包目录的 `dir` 下。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Content {
    /// 给哪个包：写法同包的编号。
    pub target: String,
    /// 包目录里的相对目录。
    pub dir: String,
}

/// 读 `[content]`：每一格是一个包的编号，值是只有 `dir` 的表。
pub(super) fn read(reader: &Reader<'_>, node: &Item) -> Result<Vec<Content>, Problem> {
    let Some(table) = node.as_table_like() else {
        return Err(bad(
            reader,
            Some(node),
            "content",
            "[content] must be a table".to_string(),
        ));
    };
    let mut found = Vec::new();
    for (target, item) in table.iter() {
        if !valid_name(target) {
            return Err(bad(
                reader,
                Some(item),
                target,
                format!("content.{target}: {target:?} is not a package id"),
            ));
        }
        let Some(inner) = item.as_table_like() else {
            return Err(bad(
                reader,
                Some(item),
                target,
                format!("content.{target} must be a table"),
            ));
        };
        let name = format!("content.{target}");
        reader.only(inner, &name, super::fields::CONTENT)?;
        let dir = reader.required(inner, item, &name, "dir")?;
        let Some(dir) = dir.as_str().filter(|dir| relative_dir(dir)) else {
            return Err(bad(
                reader,
                Some(dir),
                &format!("{name}.dir"),
                format!("{name}.dir must be a relative directory inside the package directory"),
            ));
        };
        found.push(Content {
            target: target.to_string(),
            dir: dir.to_string(),
        });
    }
    found.sort_by(|a, b| a.target.cmp(&b.target));
    Ok(found)
}

fn bad(reader: &Reader<'_>, at: Option<&Item>, detail: &str, message: String) -> Problem {
    reader.problem(at, Code::BadContent, detail, message)
}

#[cfg(test)]
mod tests;
