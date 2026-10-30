//! 清单里的 `[settings]`（施工 9-1 下，`docs/blueprint/packages.md`「配置项」，`05-内核接口.md` 第二节）：一项一张表
//! `[settings.<名字>]`，或者多一层组 `[settings.<组>.<名字>]`（施工 9-1 再补），读成 [`Setting`]；核心起来时照它们拼成配置项（[`items`]），键是 `<包的编号>.<名字>`，挂在设置页的
//! 「软件包」那一页、这个包那一组。
//!
//! 读的时候不留 `'static` 的字：`miyu check` 每次都照磁盘读。只有核心起来时拼配置项那一次把字留在进程里，一直用到退出
//! （配置清单的项是编译期常量的样子，包的配置项装卸要重启，留下的量有界）。

use toml_edit::{Item as Node, TableLike};

use super::reader::Reader;
use super::{Code, Problem};
use crate::item::{Applies, Control, Kind, Layer};
use crate::parse;
use crate::phrases::Phrases;
use crate::value::{Number, Value};

/// 包的配置项都在设置页的这一页（施工 F-6 上起平台接入的也在这里：「接入」页去掉了，`package-pages.md`）。头照 `config.schema`
/// 每一项的 `package` 把它们画在那个软件的信息页上。
pub const PAGE: &str = "packages";

/// 把某个包整个关掉的那一项的样子（施工 U23 末上，`miyu_session::packages_off`）：中间一段是包的编号。
pub const OFF: &str = "packages.<id>.off";

/// 文字不写 `max` 的，最多几个字符。
const TEXT_MAX: usize = 200;

/// 几行字不写 `max` 的，最多几个字符（施工 PC-1）。
const LINES_MAX: usize = 2000;

/// 读好的一项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setting {
    /// 名字：键里包的编号后面那一段，在组里的是 `<组>.<名字>`。
    pub name: String,
    /// 类型。
    pub kind: SettingKind,
    /// 默认值；没写的没有。
    pub default: Option<Value>,
    /// 能写在哪几层：只有系统配置（SU-3 上起）。
    pub layers: Vec<Layer>,
    /// 什么时候生效；不写是这个程序下次启动时（[`Applies::HeadStart`]）。
    pub applies: Applies,
    /// 给人看的名字。
    pub label: Phrases,
    /// 给人看的说明；没写的是空的。
    pub description: Phrases,
    /// 设置页不画。
    pub hidden: bool,
    /// 画成滑块（施工 PC-1，`int`、`number` 写了 `slider = true`）：一格多少；不是滑块的没有。
    pub slider: Option<Number>,
}

/// 选项里的一个（施工 PC-1，设计 53 第一节）：存的值，给人看的名字、说明（没写的是空的：名字照值画）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// 存进配置的值。
    pub value: String,
    /// 给人看的名字。
    pub name: Phrases,
    /// 给人看的说明。
    pub description: Phrases,
}

impl Choice {
    /// 只有值的一个（原来那种字的写法）。
    fn plain(value: String) -> Choice {
        Choice {
            value,
            name: Phrases::new(),
            description: Phrases::new(),
        }
    }
}

/// 一项的类型：配置清单的几种里包能用的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingKind {
    /// 开关。
    Bool,
    /// 整数，`min` 到 `max`。
    Int {
        /// 最小。
        min: i64,
        /// 最大。
        max: i64,
    },
    /// 选项：列出的几个之一，至少两个（施工 PC-1 起每一个可以带给人看的名字、说明）。
    Option(Vec<Choice>),
    /// 文字，最多 `max` 个字符。
    Text {
        /// 最多几个字符。
        max: usize,
    },
    /// 几行字（施工 PC-1，`text` 写 `lines = true`）：能换行，最多 `max` 个字符。
    Lines {
        /// 最多几个字符（换行也算一个）。
        max: usize,
    },
    /// 小数（施工 PC-1，`type = "number"`）：`min` 到 `max`，两头必写、只收整数，精度照 `step`。
    Number {
        /// 最小。
        min: i64,
        /// 最大。
        max: i64,
    },
    /// 名字。
    Name,
    /// 网址。
    Url,
    /// 密钥的引用：没有默认值。
    Secret,
    /// 列表：每一个照元素的类型，元素不能再是列表（施工 9-1 补）。
    List(Box<SettingKind>),
}

impl SettingKind {
    /// 不带借来的字的那几种，换成配置清单的类型；选项、列表没有（要 `'static` 的字、元素，[`items`] 那一次才换）。
    fn plain(&self) -> Option<Kind> {
        Some(match self {
            SettingKind::Bool => Kind::Bool,
            SettingKind::Int { min, max } => Kind::Int {
                min: *min,
                max: *max,
            },
            SettingKind::Text { max } => Kind::Text { max: *max },
            SettingKind::Lines { max } => Kind::Lines { max: *max },
            SettingKind::Number { min, max } => Kind::Float {
                min: *min,
                max: *max,
            },
            SettingKind::Name => Kind::Name,
            SettingKind::Url => Kind::Url,
            SettingKind::Secret => Kind::Secret,
            SettingKind::Option(_) | SettingKind::List(_) => return None,
        })
    }

    /// 密钥、密钥的列表：不能写默认值。
    fn holds_secrets(&self) -> bool {
        match self {
            SettingKind::Secret => true,
            SettingKind::List(element) => element.holds_secrets(),
            _ => false,
        }
    }

    /// 设置页用的控件（滑块另看 [`Setting::slider`]）。
    fn control(&self) -> Control {
        match self {
            SettingKind::Bool => Control::Toggle,
            SettingKind::Int { .. } | SettingKind::Number { .. } => Control::Number,
            SettingKind::Lines { .. } => Control::TextArea,
            SettingKind::Option(_) => Control::Select,
            SettingKind::List(_) => Control::List,
            _ => Control::Text,
        }
    }
}

/// 读 `[settings]`：照写的先后。一张表里有 `type` 或 `name` 的（空的也算）是一项，别的是一组（施工 9-1 再补）：`[settings.<组>.<名字>]`，
/// 名字读成 `<组>.<名字>`，只多一层。
pub(super) fn read(reader: &Reader<'_>, node: &Node) -> Result<Vec<Setting>, Problem> {
    let Some(table) = node.as_table_like() else {
        return Ok(Vec::new());
    };
    let mut settings = Vec::new();
    for (name, item) in table.iter() {
        let (at, fields) = entry(reader, "settings", name, item)?;
        if fields.is_empty() || fields.contains_key("type") || fields.contains_key("name") {
            settings.push(one(reader, name, &at, fields, item)?);
            continue;
        }
        for (inner, item) in fields.iter() {
            let (at, fields) = entry(reader, &at, inner, item)?;
            settings.push(one(reader, &format!("{name}.{inner}"), &at, fields, item)?);
        }
    }
    Ok(settings)
}

/// `<上一层>.<名字>` 那一张表：名字合写法、是一张表。
fn entry<'a>(
    reader: &Reader<'_>,
    above: &str,
    name: &str,
    item: &'a Node,
) -> Result<(String, &'a dyn TableLike), Problem> {
    let at = format!("{above}.{name}");
    if !setting_name(name) {
        return Err(reader.problem(
            Some(item),
            Code::BadSettingName,
            &at,
            format!("{at}: a setting name starts with a lowercase letter and uses only lowercase letters, digits and _"),
        ));
    }
    let Some(fields) = item.as_table_like() else {
        return Err(reader.problem(
            Some(item),
            Code::NotATable,
            &at,
            format!("{at} must be a table"),
        ));
    };
    Ok((at, fields))
}

/// 一项。
fn one(
    reader: &Reader<'_>,
    name: &str,
    at: &str,
    fields: &dyn TableLike,
    node: &Node,
) -> Result<Setting, Problem> {
    let kind = kind(reader, at, fields, node)?;
    let slider = slider(reader, at, fields, &kind)?;
    let default = match fields.get("default") {
        None => None,
        Some(item) => Some(default(reader, at, &kind, item)?),
    };
    let layers = match fields.get("layers") {
        None => vec![Layer::System],
        Some(item) => layers(reader, at, item)?,
    };
    let applies = match fields.get("applies") {
        None => Applies::HeadStart,
        Some(item) => match item.as_str() {
            Some("now") => Applies::Now,
            Some("new_session") => Applies::NewSession,
            Some("next_turn") => Applies::NextTurn,
            Some("program_start") => Applies::HeadStart,
            _ => {
                return Err(reader.problem(
                    Some(item),
                    Code::BadApplies,
                    &format!("{at}.applies"),
                    format!("{at}.applies must be now, new_session, next_turn or program_start"),
                ));
            }
        },
    };
    let hidden = match fields.get("hidden") {
        None => false,
        Some(item) => item.as_bool().ok_or_else(|| {
            reader.problem(
                Some(item),
                Code::NotBool,
                &format!("{at}.hidden"),
                format!("{at}.hidden must be true or false"),
            )
        })?,
    };
    let label = reader.phrases(
        reader.required(fields, node, at, "name")?,
        &format!("{at}.name"),
    )?;
    let description = match fields.get("description") {
        None => Phrases::new(),
        Some(item) => reader.phrases(item, &format!("{at}.description"))?,
    };
    Ok(Setting {
        name: name.to_string(),
        kind,
        default,
        layers,
        applies,
        label,
        description,
        hidden,
        slider,
    })
}

/// `type` 和它带的几格：`min`、`max`（整数）、`choices`（选项）、`max`（文字）；列表多 `element`，这几格照元素的类型收。
/// 别的类型写了这几格的报不认识的键。
fn kind(
    reader: &Reader<'_>,
    at: &str,
    fields: &dyn TableLike,
    node: &Node,
) -> Result<SettingKind, Problem> {
    let item = reader.required(fields, node, at, "type")?;
    let (kind, extra, list) = if item.as_str() == Some("list") {
        let element = reader.required(fields, node, at, "element")?;
        let bad = || {
            reader.problem(
                Some(element),
                Code::BadElement,
                &format!("{at}.element"),
                format!(
                    "{at}.element must be bool, int, number, option, text, name, url or secret"
                ),
            )
        };
        let name = element.as_str().ok_or_else(bad)?;
        let (inner, extra) = scalar(reader, at, fields, node, name)?.ok_or_else(bad)?;
        (SettingKind::List(Box::new(inner)), extra, true)
    } else {
        let scalar = match item.as_str() {
            Some(name) => scalar(reader, at, fields, node, name)?,
            None => None,
        };
        let (kind, extra) = scalar.ok_or_else(|| {
            reader.problem(
                Some(item),
                Code::BadType,
                &format!("{at}.type"),
                format!(
                    "{at}.type must be bool, int, number, option, text, name, url, secret or list"
                ),
            )
        })?;
        (kind, extra, false)
    };
    let element: &[&str] = if list {
        super::fields::SETTING_LIST
    } else {
        &[]
    };
    let known: Vec<&str> = super::fields::SETTING
        .iter()
        .chain(extra)
        .chain(element)
        .copied()
        .collect();
    reader.only(fields, at, &known)?;
    Ok(kind)
}

/// 默认值：照类型查；选项照列出的几个查；列表照元素一个个查；密钥、密钥的列表不能有。
fn default(
    reader: &Reader<'_>,
    at: &str,
    kind: &SettingKind,
    item: &Node,
) -> Result<Value, Problem> {
    let bad = || {
        reader.problem(
            Some(item),
            Code::BadDefault,
            &format!("{at}.default"),
            format!("{at}.default does not fit its type"),
        )
    };
    let value = item.as_value().ok_or_else(bad)?;
    if kind.holds_secrets() {
        return Err(bad());
    }
    fits(kind, value).ok_or_else(bad)
}

/// 一个值合不合这个类型，合的读成配置的值。
fn fits(kind: &SettingKind, value: &toml_edit::Value) -> Option<Value> {
    match kind {
        SettingKind::Option(choices) => value
            .as_str()
            .filter(|text| choices.iter().any(|choice| choice.value == *text))
            .map(|text| Value::Text(text.to_string().into())),
        SettingKind::List(element) => value
            .as_array()?
            .iter()
            .map(|value| fits(element, value))
            .collect::<Option<Vec<_>>>()
            .map(|values| Value::List(values.into())),
        plain => {
            let kind = plain.plain()?;
            parse::read(kind, value).filter(|value| kind.accepts(value))
        }
    }
}

/// `layers`：只认 `system`。
fn layers(reader: &Reader<'_>, at: &str, item: &Node) -> Result<Vec<Layer>, Problem> {
    let bad = || {
        reader.problem(
            Some(item),
            Code::BadLayers,
            &format!("{at}.layers"),
            format!("{at}.layers must list system"),
        )
    };
    let array = item.as_array().ok_or_else(bad)?;
    let mut layers = Vec::new();
    for value in array {
        let layer = match value.as_str() {
            Some("system") => Layer::System,
            _ => return Err(bad()),
        };
        if layers.contains(&layer) {
            return Err(bad());
        }
        layers.push(layer);
    }
    if layers.is_empty() {
        return Err(bad());
    }
    layers.sort();
    Ok(layers)
}

/// 名字：小写字母开头，只有小写字母、数字、`_`，最多 64 个。
fn setting_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && name.len() <= 64
}

mod build;
mod types;

pub use build::items;
use types::{scalar, slider};

#[cfg(test)]
mod tests;
