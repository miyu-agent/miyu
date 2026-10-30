//! 清单里一项的类型怎么读（施工 9-1 下；施工 PC-1 从 `settings.rs` 分出来，行数）：不是列表的那几种带的几格，选项的两种写法，
//! 开关那一格，滑块的步长。

use toml_edit::{Item as Node, TableLike};

use super::super::reader::Reader;
use super::super::{Code, Problem};
use super::{Choice, LINES_MAX, SettingKind, TEXT_MAX};
use crate::phrases::Phrases;
use crate::value::Number;

/// 不是列表的那几种：`name` 是哪一种、它带的几格；不认识的是 `None`。
pub(super) fn scalar(
    reader: &Reader<'_>,
    at: &str,
    fields: &dyn TableLike,
    node: &Node,
    name: &str,
) -> Result<Option<(SettingKind, &'static [&'static str])>, Problem> {
    Ok(Some(match name {
        "bool" => (SettingKind::Bool, &[]),
        "int" => {
            let bound = |key: &str, otherwise: i64| match fields.get(key) {
                None => Ok(otherwise),
                Some(item) => item.as_integer().ok_or_else(|| {
                    reader.problem(
                        Some(item),
                        Code::BadRange,
                        &format!("{at}.{key}"),
                        format!("{at}.{key} must be an integer"),
                    )
                }),
            };
            let (min, max) = (bound("min", i64::MIN)?, bound("max", i64::MAX)?);
            ordered(reader, at, fields, (min, max))?;
            (
                SettingKind::Int { min, max },
                super::super::fields::SETTING_INT,
            )
        }
        "option" => (
            SettingKind::Option(choices(reader, at, fields, node)?),
            super::super::fields::SETTING_OPTION,
        ),
        "text" => {
            // 几行字（施工 PC-1）：`lines = true`，不写 `max` 的最多 2000 个字。
            let lines = flag(reader, at, fields, "lines")?;
            let max = match fields.get("max") {
                None if lines => LINES_MAX,
                None => TEXT_MAX,
                Some(item) => item
                    .as_integer()
                    .and_then(|max| usize::try_from(max).ok())
                    .filter(|max| *max > 0)
                    .ok_or_else(|| {
                        reader.problem(
                            Some(item),
                            Code::BadRange,
                            &format!("{at}.max"),
                            format!("{at}.max must be a positive integer"),
                        )
                    })?,
            };
            let kind = match lines {
                true => SettingKind::Lines { max },
                false => SettingKind::Text { max },
            };
            (kind, super::super::fields::SETTING_TEXT)
        }
        // 小数（施工 PC-1）：两头必写、只收整数，精度照 `step`。
        "number" => {
            let bound = |key: &str| {
                let item = fields.get(key);
                item.and_then(Node::as_integer).ok_or_else(|| {
                    reader.problem(
                        item.or(Some(node)),
                        Code::BadRange,
                        &format!("{at}.{key}"),
                        format!("{at}.{key} must be written as an integer"),
                    )
                })
            };
            let (min, max) = (bound("min")?, bound("max")?);
            ordered(reader, at, fields, (min, max))?;
            (
                SettingKind::Number { min, max },
                super::super::fields::SETTING_NUMBER,
            )
        }
        "name" => (SettingKind::Name, &[]),
        "url" => (SettingKind::Url, &[]),
        "secret" => (SettingKind::Secret, &[]),
        _ => return Ok(None),
    }))
}

/// `min` 不大过 `max`。
fn ordered(
    reader: &Reader<'_>,
    at: &str,
    fields: &dyn TableLike,
    (min, max): (i64, i64),
) -> Result<(), Problem> {
    if min > max {
        return Err(reader.problem(
            fields.get("min"),
            Code::BadRange,
            &format!("{at}.min"),
            format!("{at}.min must not be above {at}.max"),
        ));
    }
    Ok(())
}

/// 开关那一格（`lines`、`slider`）：没写的是假的。
pub(super) fn flag(
    reader: &Reader<'_>,
    at: &str,
    fields: &dyn TableLike,
    key: &str,
) -> Result<bool, Problem> {
    match fields.get(key) {
        None => Ok(false),
        Some(item) => item.as_bool().ok_or_else(|| {
            reader.problem(
                Some(item),
                Code::NotBool,
                &format!("{at}.{key}"),
                format!("{at}.{key} must be true or false"),
            )
        }),
    }
}

/// `choices`（施工 PC-1 起两种写法，不能混着写）：字的数组；或者表的数组，`value` 必写（字），`name`、`description` 照项的
/// 名字、说明那样写，别的格不收。至少两个，值不重。
fn choices(
    reader: &Reader<'_>,
    at: &str,
    fields: &dyn TableLike,
    node: &Node,
) -> Result<Vec<Choice>, Problem> {
    let item = fields.get("choices");
    let bad = || {
        reader.problem(
            item.or(Some(node)),
            Code::BadChoices,
            &format!("{at}.choices"),
            format!(
                "{at}.choices must list at least two different texts, or tables with a text value and an optional name and description"
            ),
        )
    };
    let array = item.and_then(Node::as_array).ok_or_else(bad)?;
    let choices: Vec<Choice> = if array.iter().all(|value| value.as_str().is_some()) {
        array
            .iter()
            .filter_map(|value| value.as_str().map(|text| Choice::plain(text.to_string())))
            .collect()
    } else {
        let mut read = Vec::new();
        for (n, value) in array.iter().enumerate() {
            let table = value.as_inline_table().ok_or_else(bad)?;
            if table
                .iter()
                .any(|(key, _)| !matches!(key, "value" | "name" | "description"))
            {
                return Err(bad());
            }
            let value = table
                .get("value")
                .and_then(toml_edit::Value::as_str)
                .ok_or_else(bad)?;
            let words = |key: &str| match table.get(key) {
                None => Ok(Phrases::new()),
                Some(words) => reader.phrases(
                    &Node::Value(words.clone()),
                    &format!("{at}.choices[{n}].{key}"),
                ),
            };
            read.push(Choice {
                value: value.to_string(),
                name: words("name")?,
                description: words("description")?,
            });
        }
        read
    };
    let mut distinct: Vec<&str> = choices.iter().map(|choice| choice.value.as_str()).collect();
    distinct.sort_unstable();
    distinct.dedup();
    if distinct.len() < 2 || distinct.len() != choices.len() {
        return Err(bad());
    }
    Ok(choices)
}

/// 滑块（施工 PC-1）：`int`、`number` 写了 `slider = true` 的，交回一格多少；要两头都写了。`step` 写了的要是正数，`int` 的是整数
/// （不画滑块的也查，头照它画数字框的步长）；不写的 `int` 是 1、`number` 是范围的百分之一。
pub(super) fn slider(
    reader: &Reader<'_>,
    at: &str,
    fields: &dyn TableLike,
    kind: &SettingKind,
) -> Result<Option<Number>, Problem> {
    let (min, max, whole) = match kind {
        SettingKind::Int { min, max } => (*min, *max, true),
        SettingKind::Number { min, max } => (*min, *max, false),
        _ => return Ok(None),
    };
    let step = match fields.get("step") {
        None => None,
        Some(item) => {
            let step = match (item.as_integer(), item.as_float()) {
                (Some(step), _) => Some(step as f64),
                (None, Some(step)) if !whole => Some(step),
                _ => None,
            };
            let step = step
                .filter(|step| step.is_finite() && *step > 0.0)
                .ok_or_else(|| {
                    reader.problem(
                        Some(item),
                        Code::BadRange,
                        &format!("{at}.step"),
                        match whole {
                            true => format!("{at}.step must be a positive integer"),
                            false => format!("{at}.step must be a positive number"),
                        },
                    )
                })?;
            Some(step)
        }
    };
    if !flag(reader, at, fields, "slider")? {
        return Ok(None);
    }
    if fields.get("min").is_none() || fields.get("max").is_none() {
        return Err(reader.problem(
            fields.get("slider"),
            Code::BadRange,
            &format!("{at}.slider"),
            format!("{at} needs min and max to be a slider"),
        ));
    }
    let step = step.unwrap_or(match whole {
        true => 1.0,
        false => (max - min) as f64 / 100.0,
    });
    Ok(Some(Number::new(step)))
}
