//! 界面块的内容（施工 X-15 中，`docs/blueprint/widgets.md`「内容」，设计 `33-界面扩展.md` 第四节）：程序推来的一块内容
//! 查写法（[`check`]）、换掉的标题查写法（[`check_title`]）、推给头之前照连接的语言把字挑好（[`localize`]）。纯逻辑。
//!
//! 只认六种段、各自认的格；整页里另有能改的两种（`sections`、`form`，施工 X-15 再补，`form.rs`）。字是一个字符串，或者语言
//! （`zh`、`en`、`ja`）到字符串的表。写错的整块不收，交回错在哪（[`Bad`]）：核心不替程序改。

use std::collections::BTreeSet;

use miyu_config::package::WidgetPlace;
use miyu_config::phrases::LANGUAGES;
use serde_json::{Map, Value};
use unicode_width::UnicodeWidthStr;

/// 一块最多几段。
pub const MOST_PARTS: usize = 64;
/// 一块编成 JSON 最多多少字节。
pub const MOST_BYTES: usize = 64 * 1024;
/// `text` 段、`copy` 动作的字最多几个。
const LONG: usize = 2000;
/// 别的字最多几个。
const SHORT: usize = 200;
/// 列表一条右边的字最多分几段（施工 X-15 补：改动的文件「+12 −3」两种颜色）。
const RIGHT_SPANS: usize = 4;
/// 首页的一块最多几段。
const HOME_PARTS: usize = 8;
/// 底栏的一段最多多宽（显示宽度，宽字算两列）。
const STATUS_COLUMNS: usize = 40;
/// 语气。
const TONES: [&str; 6] = ["normal", "dim", "accent", "good", "warn", "bad"];
/// 一条的状态。
const STATES: [&str; 5] = ["todo", "doing", "done", "failed", "off"];

/// 错的是哪一种。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// 类型不对：不是数组、对象、字，或者段的种类不认识。
    Type,
    /// 少了必写的一格。
    Missing,
    /// 多了不认识的一格。
    Unknown,
    /// 字太长。
    TooLong,
    /// 段、条、对、按钮太多。
    TooMany,
    /// 整块编成 JSON 太大。
    TooBig,
    /// 字里有控制字符、改写方向的字符。
    ControlChar,
    /// 语气不认识。
    Tone,
    /// 状态不认识。
    State,
    /// 数不在 0 到 1。
    Value,
    /// 动作写法不对，或者打开的不是这个包的整页。
    Action,
    /// 这个位置放不下：底栏只能一段一行的 `text`，首页最多 8 段。
    Place,
    /// 语言表里有不认的语言，或者是空的。
    Language,
    /// 编号、键重了：表单的编号、一张表单里格的键、一组里项的编号、选项的值（施工 X-15 再补）。
    Duplicate,
}

impl Problem {
    /// 协议上的写法（`bad_widget_content` 的 `data.problem`）。
    pub fn as_str(self) -> &'static str {
        match self {
            Problem::Type => "type",
            Problem::Missing => "missing",
            Problem::Unknown => "unknown",
            Problem::TooLong => "too_long",
            Problem::TooMany => "too_many",
            Problem::TooBig => "too_big",
            Problem::ControlChar => "control_char",
            Problem::Tone => "tone",
            Problem::State => "state",
            Problem::Value => "value",
            Problem::Action => "action",
            Problem::Place => "place",
            Problem::Language => "language",
            Problem::Duplicate => "duplicate",
        }
    }
}

/// 写错的一处：`path` 是在哪（例如 `content[4].items[2].title`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bad {
    /// 在哪。
    pub path: String,
    /// 哪一种。
    pub problem: Problem,
}

fn bad(path: &str, problem: Problem) -> Bad {
    Bad {
        path: path.to_string(),
        problem,
    }
}

/// 查一块内容。`place` 是这一块放在哪；`pages` 是这个包的整页的编号（`open` 动作只能开它们）。
///
/// # Errors
///
/// 第一处写错的。
pub fn check(place: WidgetPlace, pages: &[&str], content: &Value) -> Result<(), Bad> {
    let Some(parts) = content.as_array() else {
        return Err(bad("content", Problem::Type));
    };
    if serde_json::to_string(content).map_or(true, |text| text.len() > MOST_BYTES) {
        return Err(bad("content", Problem::TooBig));
    }
    if parts.len() > MOST_PARTS {
        return Err(bad("content", Problem::TooMany));
    }
    match place {
        WidgetPlace::Home if parts.len() > HOME_PARTS => {
            return Err(bad("content", Problem::Place));
        }
        WidgetPlace::Status => status(parts)?,
        _ => {}
    }
    let mut ctx = Ctx {
        pages,
        page: place == WidgetPlace::Page,
        inside: false,
        sections: false,
        forms: BTreeSet::new(),
    };
    for (n, part) in parts.iter().enumerate() {
        self::part(&format!("content[{n}]"), part, &mut ctx)?;
    }
    Ok(())
}

/// 查一块时带着的（施工 X-15 再补）：这个包的整页的编号、这一块在不在整页里、是不是在 `sections` 的一项里、有没有用过
/// `sections`、见过的表单编号。
struct Ctx<'a> {
    pages: &'a [&'a str],
    page: bool,
    inside: bool,
    sections: bool,
    forms: BTreeSet<String>,
}

/// 换掉的标题：一句短的字，不换行。
///
/// # Errors
///
/// 写错的那一处，`path` 是 `title`。
pub fn check_title(title: &Value) -> Result<(), Bad> {
    words(title, "title", SHORT, false)
}

/// 照连接的语言 `language` 把内容里的字挑好：语言表照这种语言、`en`、`zh`、`ja` 的先后挑一句，字照旧。查过的内容才挑。
pub fn localize(content: &Value, language: &str) -> Value {
    let Some(parts) = content.as_array() else {
        return content.clone();
    };
    Value::Array(
        parts
            .iter()
            .map(|part| {
                let mut part = part.clone();
                localize_part(&mut part, language);
                part
            })
            .collect(),
    )
}

/// 一段照语言挑好；`sections` 一项里的段、表单里的字也挑（施工 X-15 再补，[`form::pick_inside`]）。
fn localize_part(part: &mut Value, language: &str) {
    let Some(object) = part.as_object_mut() else {
        return;
    };
    pick_in(object, &["text"], language);
    if let Some(items) = object.get_mut("items").and_then(Value::as_array_mut) {
        for item in items.iter_mut().filter_map(Value::as_object_mut) {
            pick_in(item, &["label", "value", "title", "detail"], language);
            match item.get_mut("right") {
                Some(Value::Array(spans)) => {
                    for span in spans.iter_mut().filter_map(Value::as_object_mut) {
                        pick_in(span, &["text"], language);
                    }
                }
                Some(right) => *right = pick(right, language),
                None => {}
            }
        }
    }
    form::pick_inside(object, language);
}

/// 一句字照语言挑好：字照旧，语言表挑一句。
pub fn pick(words: &Value, language: &str) -> Value {
    match words.as_object() {
        Some(table) => [language, "en", "zh", "ja"]
            .iter()
            .find_map(|language| table.get(*language))
            .cloned()
            .unwrap_or(Value::Null),
        None => words.clone(),
    }
}

fn pick_in(object: &mut Map<String, Value>, keys: &[&str], language: &str) {
    for key in keys {
        if let Some(value) = object.get_mut(*key) {
            *value = pick(value, language);
        }
    }
}

/// 底栏：只能一段 `text`，一行，每种语言的显示宽度都不超过 40 列。
fn status(parts: &[Value]) -> Result<(), Bad> {
    let [part] = parts else {
        return Err(bad("content", Problem::Place));
    };
    if part.get("type").and_then(Value::as_str) != Some("text") {
        return Err(bad("content[0]", Problem::Place));
    }
    let variants: Vec<&str> = match part.get("text") {
        Some(Value::String(text)) => vec![text.as_str()],
        Some(Value::Object(table)) => table.values().filter_map(Value::as_str).collect(),
        _ => return Ok(()),
    };
    for text in variants {
        if text.contains('\n') {
            return Err(bad("content[0].text", Problem::Place));
        }
        if text.width() > STATUS_COLUMNS {
            return Err(bad("content[0].text", Problem::TooLong));
        }
    }
    Ok(())
}

fn part(at: &str, part: &Value, ctx: &mut Ctx<'_>) -> Result<(), Bad> {
    let pages = ctx.pages;
    let Some(object) = part.as_object() else {
        return Err(bad(at, Problem::Type));
    };
    let kind = match object.get("type") {
        Some(Value::String(kind)) => kind.as_str(),
        Some(_) => return Err(bad(&format!("{at}.type"), Problem::Type)),
        None => return Err(bad(&format!("{at}.type"), Problem::Missing)),
    };
    match kind {
        "heading" => {
            only(at, object, &["type", "text"])?;
            required(at, object, "text", SHORT, false)
        }
        "text" => {
            only(at, object, &["type", "text", "tone"])?;
            required(at, object, "text", LONG, true)?;
            one_of(at, object, "tone", &TONES, Problem::Tone)
        }
        "pairs" => {
            only(at, object, &["type", "items"])?;
            items(at, object, 50, |at, item| {
                only(at, item, &["label", "value", "tone"])?;
                required(at, item, "label", SHORT, false)?;
                required(at, item, "value", SHORT, false)?;
                one_of(at, item, "tone", &TONES, Problem::Tone)
            })
        }
        "progress" => {
            only(at, object, &["type", "value", "text", "mark"])?;
            match object.get("value") {
                Some(value) => fraction(&format!("{at}.value"), value)?,
                None => return Err(bad(&format!("{at}.value"), Problem::Missing)),
            }
            if let Some(mark) = object.get("mark") {
                fraction(&format!("{at}.mark"), mark)?;
            }
            optional(at, object, "text", SHORT)
        }
        "list" => {
            only(at, object, &["type", "items"])?;
            items(at, object, 200, |at, item| {
                only(at, item, &["title", "detail", "right", "state", "action"])?;
                required(at, item, "title", SHORT, false)?;
                optional(at, item, "detail", SHORT)?;
                right(&format!("{at}.right"), item.get("right"))?;
                one_of(at, item, "state", &STATES, Problem::State)?;
                match item.get("action") {
                    Some(action) => self::action(&format!("{at}.action"), action, pages),
                    None => Ok(()),
                }
            })
        }
        "buttons" => {
            only(at, object, &["type", "items"])?;
            items(at, object, 8, |at, item| {
                only(at, item, &["label", "action"])?;
                required(at, item, "label", SHORT, false)?;
                match item.get("action") {
                    Some(action) => self::action(&format!("{at}.action"), action, pages),
                    None => Err(bad(&format!("{at}.action"), Problem::Missing)),
                }
            })
        }
        "sections" => form::sections(at, object, ctx),
        "form" => form::form(at, object, ctx),
        _ => Err(bad(&format!("{at}.type"), Problem::Type)),
    }
}

/// `items`：必写的数组，最多 `most` 项，每一项是对象，照 `each` 查。
fn items(
    at: &str,
    object: &Map<String, Value>,
    most: usize,
    each: impl Fn(&str, &Map<String, Value>) -> Result<(), Bad>,
) -> Result<(), Bad> {
    let path = format!("{at}.items");
    let list = match object.get("items") {
        Some(Value::Array(list)) => list,
        Some(_) => return Err(bad(&path, Problem::Type)),
        None => return Err(bad(&path, Problem::Missing)),
    };
    if list.len() > most {
        return Err(bad(&path, Problem::TooMany));
    }
    for (n, item) in list.iter().enumerate() {
        let at = format!("{path}[{n}]");
        let Some(item) = item.as_object() else {
            return Err(bad(&at, Problem::Type));
        };
        each(&at, item)?;
    }
    Ok(())
}

/// 列表一条右边的字：一句字（同别的字），或者最多 [`RIGHT_SPANS`] 段 `{"text", "tone"?}`，每段带自己的语气。
fn right(at: &str, value: Option<&Value>) -> Result<(), Bad> {
    let spans = match value {
        None => return Ok(()),
        Some(Value::Array(spans)) => spans,
        Some(value) => return words(value, at, SHORT, false),
    };
    if spans.is_empty() {
        return Err(bad(at, Problem::Missing));
    }
    if spans.len() > RIGHT_SPANS {
        return Err(bad(at, Problem::TooMany));
    }
    for (n, span) in spans.iter().enumerate() {
        let at = format!("{at}[{n}]");
        let Some(span) = span.as_object() else {
            return Err(bad(&at, Problem::Type));
        };
        only(&at, span, &["text", "tone"])?;
        required(&at, span, "text", SHORT, false)?;
        one_of(&at, span, "tone", &TONES, Problem::Tone)?;
    }
    Ok(())
}

/// 动作：`call`（可以带 `params` 对象）、`open`、`copy` 正好一种。
fn action(at: &str, action: &Value, pages: &[&str]) -> Result<(), Bad> {
    let Some(object) = action.as_object() else {
        return Err(bad(at, Problem::Action));
    };
    let fine = match (object.get("call"), object.get("open"), object.get("copy")) {
        (Some(Value::String(method)), None, None) => {
            method_name(method)
                && object.len() <= 2
                && object.get("params").is_none_or(Value::is_object)
                && (object.len() == 1 || object.contains_key("params"))
        }
        (None, Some(Value::String(page)), None) => {
            object.len() == 1 && pages.contains(&page.as_str())
        }
        (None, None, Some(Value::String(text))) => {
            object.len() == 1 && text.chars().count() <= LONG && clean(text, true)
        }
        _ => false,
    };
    if fine {
        Ok(())
    } else {
        Err(bad(at, Problem::Action))
    }
}

/// 程序登记的方法名的写法（`package-pages.md` 的 `package.methods`）：小写字母开头，小写字母、数字、`_`、`.`、`-`，
/// 1 到 64 个。
fn method_name(name: &str) -> bool {
    name.len() <= 64
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | '-'))
}

fn fraction(at: &str, value: &Value) -> Result<(), Bad> {
    match value.as_f64() {
        Some(number) if (0.0..=1.0).contains(&number) => Ok(()),
        Some(_) => Err(bad(at, Problem::Value)),
        None => Err(bad(at, Problem::Type)),
    }
}

fn only(at: &str, object: &Map<String, Value>, keys: &[&str]) -> Result<(), Bad> {
    match object.keys().find(|key| !keys.contains(&key.as_str())) {
        Some(key) => Err(bad(&format!("{at}.{key}"), Problem::Unknown)),
        None => Ok(()),
    }
}

fn required(
    at: &str,
    object: &Map<String, Value>,
    key: &str,
    most: usize,
    lines: bool,
) -> Result<(), Bad> {
    match object.get(key) {
        Some(value) => words(value, &format!("{at}.{key}"), most, lines),
        None => Err(bad(&format!("{at}.{key}"), Problem::Missing)),
    }
}

fn optional(at: &str, object: &Map<String, Value>, key: &str, most: usize) -> Result<(), Bad> {
    match object.get(key) {
        Some(value) => words(value, &format!("{at}.{key}"), most, false),
        None => Ok(()),
    }
}

fn one_of(
    at: &str,
    object: &Map<String, Value>,
    key: &str,
    allowed: &[&str],
    problem: Problem,
) -> Result<(), Bad> {
    match object.get(key) {
        None => Ok(()),
        Some(Value::String(text)) if allowed.contains(&text.as_str()) => Ok(()),
        Some(_) => Err(bad(&format!("{at}.{key}"), problem)),
    }
}

/// 一处字：字符串，或者语言到字符串的表（不认的语言、空表都不收）；最多 `most` 个字，`lines` 的能换行。
fn words(value: &Value, at: &str, most: usize, lines: bool) -> Result<(), Bad> {
    let one = |text: &str, at: &str| {
        if text.chars().count() > most {
            Err(bad(at, Problem::TooLong))
        } else if !clean(text, lines) {
            Err(bad(at, Problem::ControlChar))
        } else {
            Ok(())
        }
    };
    match value {
        Value::String(text) => one(text, at),
        Value::Object(table) if table.is_empty() => Err(bad(at, Problem::Language)),
        Value::Object(table) => {
            for (language, text) in table {
                let at = format!("{at}.{language}");
                if !LANGUAGES.contains(&language.as_str()) {
                    return Err(bad(&at, Problem::Language));
                }
                let Value::String(text) = text else {
                    return Err(bad(&at, Problem::Type));
                };
                one(text, &at)?;
            }
            Ok(())
        }
        _ => Err(bad(at, Problem::Type)),
    }
}

/// 没有控制字符（`lines` 的能有换行）、没有改写方向的字符：防程序拿它们仿冒界面自己的字。
fn clean(text: &str, lines: bool) -> bool {
    text.chars().all(|c| {
        if c == '\n' {
            return lines;
        }
        !(c.is_control() || matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'))
    })
}

mod form;
#[cfg(test)]
mod tests;
