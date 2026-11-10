//! 设置页上一个包的分节（施工 SG-1，`docs/blueprint/packages.md`「配置项」第 1 条）：组表 `[settings.<组>]` 自己可以写两格，
//! `name`（语言表，写法同一项的名字）和 `shown_when`（`{ "<这个包的一项>" = "<值>" }`，值也可以是几个的列表）。头照它分节画、
//! 照那一项现在的值决定画不画；核心只读、只查写得对不对，原样交 `config.schema`，存、读、查值都不看它。

use toml_edit::{Item as Node, TableLike};

use super::super::reader::Reader;
use super::super::{Code, Problem};
use super::{Setting, SettingKind};
use crate::phrases::Phrases;

/// 一节：清单里的一个组，照写的先后。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// 组名：`[settings.<组>]` 的 `<组>`，也是组里每一项名字的头一段。
    pub id: String,
    /// 给人看的名字；没写的是空的（`config.schema` 照编号写）。
    pub name: Phrases,
    /// 什么时候画；没写的总是画。
    pub shown_when: Option<ShownWhen>,
}

/// 照哪一项的值画一节。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownWhen {
    /// 看哪一项：这个包自己的名字（键里前缀后面那一段，例如 `engine`、`qq.mode`），只能是选项、不在这一节里。
    pub setting: String,
    /// 那一项是这几个之一时画，照写的先后；清单写一个的也是一个的列表。每一个都是那一项的选项。
    pub values: Vec<String>,
}

/// 读组 `id`（表叫 `at`）自己的两格。项在不在、`shown_when` 指的是哪一项，读完全部的项以后照 [`check`] 查。
pub(super) fn read(
    reader: &Reader<'_>,
    id: &str,
    at: &str,
    fields: &dyn TableLike,
) -> Result<Section, Problem> {
    let name = match fields.get("name") {
        None => Phrases::new(),
        Some(item) => reader.phrases(item, &format!("{at}.name"))?,
    };
    let shown_when = match fields.get("shown_when") {
        None => None,
        Some(item) => Some(shown_when(reader, at, item)?),
    };
    Ok(Section {
        id: id.to_string(),
        name,
        shown_when,
    })
}

/// `shown_when`：一张只有一格的表，值是一个字或者几个字的列表（不能是空的）。
fn shown_when(reader: &Reader<'_>, at: &str, item: &Node) -> Result<ShownWhen, Problem> {
    let bad = || {
        let why = "must map one setting to a value or a list of values";
        located(reader, Some(item), at, why)
    };
    let table = item.as_table_like().ok_or_else(bad)?;
    let mut entries = table.iter();
    let (Some((setting, value)), None) = (entries.next(), entries.next()) else {
        return Err(bad());
    };
    let text = |value: &toml_edit::Value| value.as_str().map(str::to_string);
    let values = match value.as_value() {
        Some(toml_edit::Value::Array(array)) => array.iter().map(text).collect(),
        Some(one) => text(one).map(|one| vec![one]),
        None => None,
    }
    .filter(|values| !values.is_empty())
    .ok_or_else(bad)?;
    Ok(ShownWhen {
        setting: setting.to_string(),
        values,
    })
}

/// 每一节的 `shown_when` 指的是这个包自己的一项（名字照 `settings` 认，别的包的键、写全了前缀的都不是）、是选项、不在这一节里，
/// 值都是它的选项。`table` 是整张 `[settings]`，报错时照它找回那一格的行号。
pub(super) fn check(
    reader: &Reader<'_>,
    table: &dyn TableLike,
    sections: &[Section],
    settings: &[Setting],
) -> Result<(), Problem> {
    for section in sections {
        let Some(when) = &section.shown_when else {
            continue;
        };
        let at = format!("settings.{}", section.id);
        let item = table
            .get(&section.id)
            .and_then(Node::as_table_like)
            .and_then(|group| group.get("shown_when"));
        let Some(target) = settings.iter().find(|one| one.name == when.setting) else {
            let why = format!("{} is not a setting of this package", when.setting);
            return Err(located(reader, item, &at, &why));
        };
        if target.section() == Some(section.id.as_str()) {
            let why = format!("{} is inside this group itself", when.setting);
            return Err(located(reader, item, &at, &why));
        }
        let SettingKind::Option(choices) = &target.kind else {
            let why = format!("{} is not an option", when.setting);
            return Err(located(reader, item, &at, &why));
        };
        if let Some(value) = when
            .values
            .iter()
            .find(|value| !choices.iter().any(|choice| &choice.value == *value))
        {
            let why = format!("{value} is not a choice of {}", when.setting);
            return Err(located(reader, item, &at, &why));
        }
    }
    Ok(())
}

/// `<at>.shown_when` 写错了：`why` 是英文的半句；`item` 是那一格（找不回来的不带行号）。
fn located(reader: &Reader<'_>, item: Option<&Node>, at: &str, why: &str) -> Problem {
    reader.problem(
        item,
        Code::BadShownWhen,
        &format!("{at}.shown_when"),
        format!("{at}.shown_when: {why}"),
    )
}
