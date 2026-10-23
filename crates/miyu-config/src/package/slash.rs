//! 扩展声明的斜杠命令（施工 X-14 下，`docs/blueprint/slash.md`「扩展的命令」）：`[[slash]]` 一段一条，读成 [`SlashCommand`]。
//! 只查写法；人敲了怎么交给扩展的程序在核心那一边。只有扩展包能写（`kinds::tables`），要在 `[process] capabilities` 里声明
//! `commands`（[`needs_capability`]，读完整份以后查）。

use toml_edit::Item;

use super::capability::Capability;
use super::reader::{Reader, command_name, line_of};
use super::{Code, Problem};
use crate::phrases::Phrases;

/// 一条斜杠命令谁能用（施工 O-6 六补，`docs/blueprint/slash.md`「谁能用」）：系统配置 `commands.<名字>.access`、清单
/// `[[slash]]` 的出厂值都是这三种。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// 本人、对应表里认得的外部身份、场所里管理的人（照旧，不写的就是它）。
    Admin,
    /// 谁都行：场所里随便哪个外部身份。命令自己的限制照旧（`/workspace` 只给主人本人）。
    Anyone,
    /// 谁都不行：回 `command_not_allowed`。
    Off,
}

impl Access {
    /// 照写法认：`admin`、`anyone`、`off`；别的没有。
    pub fn parse(text: &str) -> Option<Access> {
        match text {
            "admin" => Some(Access::Admin),
            "anyone" => Some(Access::Anyone),
            "off" => Some(Access::Off),
            _ => None,
        }
    }
}

/// 扩展声明的一条斜杠命令。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCommand {
    /// 命令名：写法同子命令名（小写字母开头，小写字母、数字、`-`，最多 32 个）。
    pub name: String,
    /// 一句说明，命令菜单里给人看的，照语言。
    pub summary: Phrases,
    /// 名字后面跟什么的提示，照语言；不跟字的没有。
    pub argument: Option<Phrases>,
    /// 出厂的谁能用（施工 O-6 六补）：没写的没有，照 `admin`；系统配置写了的压过它。
    pub access: Option<Access>,
    /// 这一段写在第几行。
    pub line: Option<usize>,
}

impl SlashCommand {
    /// 它的功能的编号：`command-<名字>`（同模板命令，预设照它开关）。
    pub fn feature_id(&self) -> String {
        format!("command-{}", self.name)
    }
}

/// `[[slash]]`：一段一条，名字在一个包里不重。
pub(super) fn read(reader: &Reader<'_>, node: &Item) -> Result<Vec<SlashCommand>, Problem> {
    let Some(array) = node.as_array_of_tables() else {
        return Err(reader.problem(
            Some(node),
            Code::NotATable,
            "slash",
            "slash is written as [[slash]], one table per command".to_string(),
        ));
    };
    let mut commands: Vec<SlashCommand> = Vec::new();
    for table in array.iter() {
        let line = table.span().map(|span| line_of(reader.text, span.start));
        reader.only(table, "slash", super::fields::SLASH)?;
        let name = match table.get("name").and_then(Item::as_str) {
            Some(name) if command_name(name) => name.to_string(),
            Some(name) => {
                return Err(bad(
                    line,
                    &format!("slash.name {name:?}"),
                    format!(
                        "slash command name {name:?} must start with a lowercase letter, use only lowercase letters, digits and -, at most 32"
                    ),
                ));
            }
            None => {
                return Err(bad(
                    line,
                    "slash.name",
                    "slash.name is missing or not text".to_string(),
                ));
            }
        };
        let at = format!("slash.{name}");
        if commands.iter().any(|seen| seen.name == name) {
            return Err(bad(
                line,
                &at,
                format!("slash command {name:?} is written twice"),
            ));
        }
        let summary = match table.get("summary") {
            Some(item) => reader.phrases(item, &format!("{at}.summary"))?,
            None => {
                return Err(bad(
                    line,
                    &format!("{at}.summary"),
                    format!("{at}.summary is missing"),
                ));
            }
        };
        let argument = match table.get("argument") {
            Some(item) => Some(reader.phrases(item, &format!("{at}.argument"))?),
            None => None,
        };
        let access = match table.get("access") {
            None => None,
            Some(item) => match item.as_str().and_then(Access::parse) {
                Some(access) => Some(access),
                None => {
                    return Err(bad(
                        line,
                        &format!("{at}.access"),
                        format!("{at}.access must be \"admin\", \"anyone\" or \"off\""),
                    ));
                }
            },
        };
        commands.push(SlashCommand {
            name,
            summary,
            argument,
            access,
            line,
        });
    }
    Ok(commands)
}

/// 声明了斜杠命令、`[process] capabilities` 里却没有 `commands` 的：报在第一段。
pub(super) fn needs_capability(
    commands: &[SlashCommand],
    capabilities: &[Capability],
) -> Result<(), Problem> {
    match commands.first() {
        Some(first) if !capabilities.contains(&Capability::Commands) => Err(bad(
            first.line,
            "slash",
            "[[slash]] needs the commands capability in [process] capabilities".to_string(),
        )),
        _ => Ok(()),
    }
}

/// 写错了的一处。
fn bad(line: Option<usize>, detail: &str, message: String) -> Problem {
    Problem {
        line,
        code: Code::BadSlash,
        detail: detail.to_string(),
        message,
    }
}

#[cfg(test)]
mod tests;
