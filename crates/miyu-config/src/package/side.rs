//! 扩展登记的旁路请求（施工 Q-3，设计 `43-旁路请求和档位池.md` 第三节第 2 条）：`[[side]]` 一段一条，读成 [`SideRequest`]。只查
//! 写法：编号、档位、名字、说明。核心照档位从三个固定池取（`miyu_models::side`），扩展的 `model.call` 写 `side` 只认自己包里
//! 登记的编号。只有扩展包能写（`kinds::tables`）；`model.call` 谁都调得了，不另要能力。

use toml_edit::Item;

use super::reader::{Reader, command_name, line_of};
use super::{Code, Problem};
use crate::phrases::Phrases;

/// 档位：照三个固定池的名字（`miyu_models::pools::FIXED`，那边的测试守着两份一样）。
pub const SIDE_TIERS: [&str; 3] = ["lite", "normal", "flagship"];

/// 扩展登记的一条旁路请求。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideRequest {
    /// 编号：写法同命令名（小写字母开头，小写字母、数字、`-`，最多 32 个），一个包里不重。
    pub id: String,
    /// 默认要哪一档：[`SIDE_TIERS`] 之一。
    pub tier: &'static str,
    /// 给人看的名字，照语言。
    pub name: Phrases,
    /// 一句说明，照语言。
    pub summary: Phrases,
    /// 这一段写在第几行。
    pub line: Option<usize>,
}

impl SideRequest {
    /// 全编号：`<包>/<编号>`，和核心出厂的（`core/title`）一个写法。
    pub fn full_id(&self, package: &str) -> String {
        format!("{package}/{}", self.id)
    }
}

/// `[[side]]`：一段一条，编号在一个包里不重。
pub(super) fn read(reader: &Reader<'_>, node: &Item) -> Result<Vec<SideRequest>, Problem> {
    let Some(array) = node.as_array_of_tables() else {
        return Err(reader.problem(
            Some(node),
            Code::NotATable,
            "side",
            "side is written as [[side]], one table per request".to_string(),
        ));
    };
    let mut requests: Vec<SideRequest> = Vec::new();
    for table in array.iter() {
        let line = table.span().map(|span| line_of(reader.text, span.start));
        reader.only(table, "side", super::fields::SIDE)?;
        let id = match table.get("id").and_then(Item::as_str) {
            Some(id) if command_name(id) => id.to_string(),
            Some(id) => {
                return Err(bad(
                    line,
                    &format!("side.id {id:?}"),
                    format!(
                        "side request id {id:?} must start with a lowercase letter, use only lowercase letters, digits and -, at most 32"
                    ),
                ));
            }
            None => {
                return Err(bad(
                    line,
                    "side.id",
                    "side.id is missing or not text".to_string(),
                ));
            }
        };
        let at = format!("side.{id}");
        if requests.iter().any(|seen| seen.id == id) {
            return Err(bad(
                line,
                &at,
                format!("side request {id:?} is written twice"),
            ));
        }
        let tier = match table
            .get("tier")
            .and_then(Item::as_str)
            .and_then(|tier| SIDE_TIERS.into_iter().find(|known| *known == tier))
        {
            Some(tier) => tier,
            None => {
                return Err(bad(
                    line,
                    &format!("{at}.tier"),
                    format!("{at}.tier must be \"lite\", \"normal\" or \"flagship\""),
                ));
            }
        };
        let phrases = |key: &str| match table.get(key) {
            Some(item) => reader.phrases(item, &format!("{at}.{key}")),
            None => Err(bad(
                line,
                &format!("{at}.{key}"),
                format!("{at}.{key} is missing"),
            )),
        };
        let name = phrases("name")?;
        let summary = phrases("summary")?;
        requests.push(SideRequest {
            id,
            tier,
            name,
            summary,
            line,
        });
    }
    Ok(requests)
}

/// 写错了的一处。
fn bad(line: Option<usize>, key: &str, message: String) -> Problem {
    Problem {
        line,
        code: Code::BadSide,
        detail: key.to_string(),
        message,
    }
}

#[cfg(test)]
mod tests;
