//! 扩展声明的用量用途（施工 UP-3，`docs/blueprint/packages.md`「清单的格式」）：`[[purpose]]` 一段一条，读成 [`UsagePurpose`]：
//! 包调 `model.call` 写的 `purpose` 叫什么，`usage.query` 照 `purpose` 分组时给人看的 `purpose_name` 照它给。只查写法：编号、名字。
//! 只有扩展包能写（`kinds::tables`）；不另要能力。

use toml_edit::Item;

use super::reader::{Reader, line_of};
use super::{Code, Problem};
use crate::phrases::Phrases;

/// 用途最多几个字节：同 `model.call` 的 `purpose`。
const MOST: usize = 32;

/// 扩展声明的一种用途。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsagePurpose {
    /// 用途，`model.call` 写的那个：写法见 [`purpose_written`]，一个包里不重。
    pub id: String,
    /// 给人看的名字，照语言：软件说法的名词短语（「入群审核」）。
    pub name: Phrases,
    /// 这一段写在第几行。
    pub line: Option<usize>,
}

/// 用途合不合写法（`model.call` 的 `purpose` 同一个规矩）：1 到 32 个字节，只有小写字母、数字、`-`。
pub fn purpose_written(purpose: &str) -> bool {
    (1..=MOST).contains(&purpose.len())
        && purpose
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// `[[purpose]]`：一段一条，编号在一个包里不重；没写的是空的。
pub(super) fn read(reader: &Reader<'_>, node: Option<&Item>) -> Result<Vec<UsagePurpose>, Problem> {
    let Some(node) = node else {
        return Ok(Vec::new());
    };
    let Some(array) = node.as_array_of_tables() else {
        return Err(reader.problem(
            Some(node),
            Code::NotATable,
            "purpose",
            "purpose is written as [[purpose]], one table per purpose".to_string(),
        ));
    };
    let mut purposes: Vec<UsagePurpose> = Vec::new();
    for table in array.iter() {
        let line = table.span().map(|span| line_of(reader.text, span.start));
        reader.only(table, "purpose", super::fields::PURPOSE)?;
        let id = match table.get("id").and_then(Item::as_str) {
            Some(id) if purpose_written(id) => id.to_string(),
            Some(id) => {
                return Err(bad(
                    line,
                    &format!("purpose.id {id:?}"),
                    format!("purpose id {id:?} must be 1 to 32 lowercase letters, digits and -"),
                ));
            }
            None => {
                return Err(bad(
                    line,
                    "purpose.id",
                    "purpose.id is missing or not text".to_string(),
                ));
            }
        };
        let at = format!("purpose.{id}");
        if purposes.iter().any(|seen| seen.id == id) {
            return Err(bad(line, &at, format!("purpose {id:?} is written twice")));
        }
        let name = match table.get("name") {
            Some(item) => reader.phrases(item, &format!("{at}.name"))?,
            None => {
                return Err(bad(
                    line,
                    &format!("{at}.name"),
                    format!("{at}.name is missing"),
                ));
            }
        };
        purposes.push(UsagePurpose { id, name, line });
    }
    Ok(purposes)
}

/// 写错了的一处。
fn bad(line: Option<usize>, key: &str, message: String) -> Problem {
    Problem {
        line,
        code: Code::BadPurpose,
        detail: key.to_string(),
        message,
    }
}

#[cfg(test)]
mod tests;
