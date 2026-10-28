//! 界面块（施工 X-15 上，`docs/blueprint/widgets.md`「清单」，设计 `33-界面扩展.md`）：`[[widget]]` 一段一块，读成
//! [`Widget`]。只查写法；内容由包的程序推、核心转给头在 `miyu-endpoint`。只有扩展包能写（`kinds::tables`），要在
//! `[process] capabilities` 里声明 `widgets`（[`check`]，读完整份以后查）。

use toml_edit::Item;

use super::capability::Capability;
use super::reader::{Reader, command_name, line_of};
use super::slash::SlashCommand;
use super::{Code, Manifest, Problem};
use crate::phrases::Phrases;

/// 一个包最多几块。
pub const MOST: usize = 16;

/// 放在哪（设计 33 第三节）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// 首页。
    Home,
    /// 侧边栏。
    Sidebar,
    /// 底栏的一小段。
    Status,
    /// 一整页。
    Page,
}

impl Place {
    /// 清单、协议上的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Place::Home => "home",
            Place::Sidebar => "sidebar",
            Place::Status => "status",
            Place::Page => "page",
        }
    }

    /// 照写法认；不认识的没有。
    pub fn parse(text: &str) -> Option<Place> {
        [Place::Home, Place::Sidebar, Place::Status, Place::Page]
            .into_iter()
            .find(|place| place.as_str() == text)
    }
}

/// 一个包声明的一块。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Widget {
    /// 包里唯一的编号：写法同子命令名。
    pub id: String,
    /// 放在哪。
    pub place: Place,
    /// 标题，照语言；侧边栏、整页必写，首页、底栏可以没有。
    pub title: Option<Phrases>,
    /// 跟着会话（只给侧边栏）；不写的是全局的。
    pub per_session: bool,
    /// 打开这一整页的斜杠命令名（只给整页）。
    pub command: Option<String>,
    /// 内容来自包在会话里的状态（施工 TW-1，设计 46）：只给跟着会话的块。
    pub state: bool,
    /// 这一段写在第几行。
    pub line: Option<usize>,
}

impl Manifest {
    /// 这个包占的斜杠命令名，照写的先后：`[[slash]]` 的，再是整页的命令名（施工 X-15 上：和斜杠命令一个名字空间）。
    pub fn slash_names(&self) -> impl Iterator<Item = &str> {
        self.slash
            .iter()
            .map(|command| command.name.as_str())
            .chain(
                self.widgets
                    .iter()
                    .filter_map(|widget| widget.command.as_deref()),
            )
    }
}

/// `[[widget]]`：一段一块，编号在一个包里不重，最多 [`MOST`] 块。
pub(super) fn read(reader: &Reader<'_>, node: &Item) -> Result<Vec<Widget>, Problem> {
    let Some(array) = node.as_array_of_tables() else {
        return Err(reader.problem(
            Some(node),
            Code::NotATable,
            "widget",
            "widget is written as [[widget]], one table per widget".to_string(),
        ));
    };
    let mut widgets: Vec<Widget> = Vec::new();
    for table in array.iter() {
        let line = table.span().map(|span| line_of(reader.text, span.start));
        if widgets.len() == MOST {
            return Err(bad(
                line,
                "widget",
                format!("a package declares at most {MOST} widgets"),
            ));
        }
        reader.only(table, "widget", super::fields::WIDGET)?;
        let id = match table.get("id").and_then(Item::as_str) {
            Some(id) if command_name(id) => id.to_string(),
            Some(id) => {
                return Err(bad(
                    line,
                    &format!("widget.id {id:?}"),
                    format!(
                        "widget id {id:?} must start with a lowercase letter, use only lowercase letters, digits and -, at most 32"
                    ),
                ));
            }
            None => {
                return Err(bad(
                    line,
                    "widget.id",
                    "widget.id is missing or not text".to_string(),
                ));
            }
        };
        let at = format!("widget.{id}");
        if widgets.iter().any(|seen| seen.id == id) {
            return Err(bad(line, &at, format!("widget {id:?} is written twice")));
        }
        let place = match table.get("place").and_then(Item::as_str) {
            Some(text) => Place::parse(text).ok_or_else(|| {
                bad(
                    line,
                    &format!("{at}.place"),
                    format!("{at}.place must be home, sidebar, status or page, not {text:?}"),
                )
            })?,
            None => {
                return Err(bad(
                    line,
                    &format!("{at}.place"),
                    format!("{at}.place is missing or not text"),
                ));
            }
        };
        let title = match table.get("title") {
            Some(item) => Some(reader.phrases(item, &format!("{at}.title"))?),
            None if matches!(place, Place::Sidebar | Place::Page) => {
                return Err(bad(
                    line,
                    &format!("{at}.title"),
                    format!(
                        "{at}.title is missing: a {} widget needs one",
                        place.as_str()
                    ),
                ));
            }
            None => None,
        };
        let per_session = match table.get("scope").map(Item::as_str) {
            None | Some(Some("global")) => false,
            Some(Some("session")) if place == Place::Sidebar => true,
            Some(Some("session")) => {
                return Err(bad(
                    line,
                    &format!("{at}.scope"),
                    format!("{at}.scope = \"session\" is only for sidebar widgets"),
                ));
            }
            Some(_) => {
                return Err(bad(
                    line,
                    &format!("{at}.scope"),
                    format!("{at}.scope must be global or session"),
                ));
            }
        };
        let command = match table.get("command").map(Item::as_str) {
            None => None,
            Some(Some(name)) if place == Place::Page && command_name(name) => {
                Some(name.to_string())
            }
            Some(_) if place != Place::Page => {
                return Err(bad(
                    line,
                    &format!("{at}.command"),
                    format!("{at}.command is only for page widgets"),
                ));
            }
            Some(_) => {
                return Err(bad(
                    line,
                    &format!("{at}.command"),
                    format!(
                        "{at}.command must start with a lowercase letter, use only lowercase letters, digits and -, at most 32"
                    ),
                ));
            }
        };
        let state = match table.get("state").map(Item::as_bool) {
            None | Some(Some(false)) => false,
            Some(Some(true)) if per_session => true,
            Some(Some(true)) => {
                return Err(bad(
                    line,
                    &format!("{at}.state"),
                    format!("{at}.state = true is only for widgets with scope = \"session\""),
                ));
            }
            Some(None) => {
                return Err(bad(
                    line,
                    &format!("{at}.state"),
                    format!("{at}.state must be true or false"),
                ));
            }
        };
        widgets.push(Widget {
            id,
            place,
            title,
            per_session,
            command,
            state,
            line,
        });
    }
    Ok(widgets)
}

/// 读完整份以后查：声明了块、`[process] capabilities` 里却没有 `widgets` 的报在第一段；整页的命令名和这个包的
/// `[[slash]]`、别的整页撞了的报在那一段。
pub(super) fn check(
    widgets: &[Widget],
    capabilities: &[Capability],
    slash: &[SlashCommand],
) -> Result<(), Problem> {
    if let Some(first) = widgets.first()
        && !capabilities.contains(&Capability::Widgets)
    {
        return Err(bad(
            first.line,
            "widget",
            "[[widget]] needs the widgets capability in [process] capabilities".to_string(),
        ));
    }
    let mut names: Vec<&str> = slash.iter().map(|command| command.name.as_str()).collect();
    for widget in widgets {
        let Some(name) = widget.command.as_deref() else {
            continue;
        };
        if names.contains(&name) {
            return Err(bad(
                widget.line,
                &format!("widget.{}.command", widget.id),
                format!("slash command {name:?} is declared twice in this package"),
            ));
        }
        names.push(name);
    }
    Ok(())
}

/// 写错了的一处。
fn bad(line: Option<usize>, detail: &str, message: String) -> Problem {
    Problem {
        line,
        code: Code::BadWidget,
        detail: detail.to_string(),
        message,
    }
}

#[cfg(test)]
mod tests;
