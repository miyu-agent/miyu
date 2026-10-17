//! 整页里能改的两种段（施工 X-15 再补，设计 `33-界面扩展.md` 第十节，`docs/blueprint/widgets.md`「整页里能改」）：
//! `sections`（一组可选的项，头画成左边列表、右边详情）和 `form`（一张表单，保存经 `package.call` 交给放这一页的包）。
//! 只在整页里；`sections` 一块里最多一个、不嵌套；表单的编号在一块里不重，一张表单里格的键不重，一组里项的编号不重。

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use super::{
    Bad, Ctx, LONG, Problem, SHORT, TONES, bad, clean, method_name, one_of, only, optional, part,
    required, words,
};

/// `sections` 最多几项。
const MOST_ITEMS: usize = 200;
/// 一项的 `blocks` 最多几段。
const MOST_BLOCKS: usize = 32;
/// 一张表单最多几格。
const MOST_FIELDS: usize = 50;
/// 下拉的选项、`list` 的值最多几个。
const MOST_OPTIONS: usize = 100;
/// 角标最多几个字。
const BADGE: usize = 20;
/// 编号、键最多几个字。
const ID: usize = 64;
/// 一项的状态：好、要留意、坏、关着，头画成状态点。
const ITEM_STATES: [&str; 4] = ["good", "warn", "bad", "off"];
/// 控件，名字、画法同配置页的（`config.md`「控件」）。
const CONTROLS: [&str; 6] = ["toggle", "select", "text", "textarea", "list", "number"];

/// `sections`：只在整页的顶层，一块里最多一个。
pub(super) fn sections(
    at: &str,
    object: &Map<String, Value>,
    ctx: &mut Ctx<'_>,
) -> Result<(), Bad> {
    if !ctx.page || ctx.inside || ctx.sections {
        return Err(bad(at, Problem::Place));
    }
    ctx.sections = true;
    only(at, object, &["type", "items", "add"])?;
    let path = format!("{at}.items");
    let list = array(&path, object.get("items"), MOST_ITEMS)?;
    let mut ids = BTreeSet::new();
    for (n, item) in list.iter().enumerate() {
        let at = format!("{path}[{n}]");
        let Some(item) = item.as_object() else {
            return Err(bad(&at, Problem::Type));
        };
        only(
            &at,
            item,
            &["id", "title", "subtitle", "badge", "state", "blocks"],
        )?;
        id(&at, item, "id", &mut ids)?;
        required(&at, item, "title", SHORT, false)?;
        optional(&at, item, "subtitle", SHORT)?;
        badge(&format!("{at}.badge"), item.get("badge"))?;
        one_of(&at, item, "state", &ITEM_STATES, Problem::State)?;
        let blocks_at = format!("{at}.blocks");
        let blocks = array(&blocks_at, item.get("blocks"), MOST_BLOCKS)?;
        ctx.inside = true;
        for (k, block) in blocks.iter().enumerate() {
            part(&format!("{blocks_at}[{k}]"), block, ctx)?;
        }
        ctx.inside = false;
    }
    match object.get("add") {
        Some(add) => add_one(&format!("{at}.add"), add),
        None => Ok(()),
    }
}

/// `form`：只在整页里，编号在一块里不重。
pub(super) fn form(at: &str, object: &Map<String, Value>, ctx: &mut Ctx<'_>) -> Result<(), Bad> {
    if !ctx.page {
        return Err(bad(at, Problem::Place));
    }
    only(at, object, &["type", "id", "fields", "submit"])?;
    id(at, object, "id", &mut ctx.forms)?;
    let path = format!("{at}.fields");
    let fields = array(&path, object.get("fields"), MOST_FIELDS)?;
    let mut keys = BTreeSet::new();
    for (n, one) in fields.iter().enumerate() {
        let at = format!("{path}[{n}]");
        let Some(one) = one.as_object() else {
            return Err(bad(&at, Problem::Type));
        };
        field(&at, one, &mut keys)?;
    }
    let submit_at = format!("{at}.submit");
    let Some(submit) = object.get("submit") else {
        return Err(bad(&submit_at, Problem::Missing));
    };
    let Some(submit) = submit.as_object() else {
        return Err(bad(&submit_at, Problem::Type));
    };
    only(&submit_at, submit, &["label", "call", "params"])?;
    required(&submit_at, submit, "label", SHORT, false)?;
    call(&submit_at, submit, true)
}

/// 一格：键、标签、控件、值（照控件的类型，或者 `null` 沿用默认），下拉才有选项，数字才有上下限。
fn field(at: &str, object: &Map<String, Value>, keys: &mut BTreeSet<String>) -> Result<(), Bad> {
    only(
        at,
        object,
        &[
            "key",
            "label",
            "hint",
            "control",
            "value",
            "options",
            "placeholder",
            "default",
            "min",
            "max",
            "integer",
        ],
    )?;
    id(at, object, "key", keys)?;
    required(at, object, "label", SHORT, false)?;
    optional(at, object, "hint", SHORT)?;
    optional(at, object, "placeholder", SHORT)?;
    if let Some(default) = object.get("default") {
        let at = format!("{at}.default");
        let Some(default) = default.as_object() else {
            return Err(bad(&at, Problem::Type));
        };
        only(&at, default, &["text"])?;
        required(&at, default, "text", SHORT, false)?;
    }
    let control_at = format!("{at}.control");
    let control = match object.get("control") {
        Some(Value::String(control)) if CONTROLS.contains(&control.as_str()) => control.as_str(),
        Some(_) => return Err(bad(&control_at, Problem::Type)),
        None => return Err(bad(&control_at, Problem::Missing)),
    };
    let options = match (control, object.get("options")) {
        ("select", Some(list)) => options(&format!("{at}.options"), list)?,
        ("select", None) => return Err(bad(&format!("{at}.options"), Problem::Missing)),
        (_, Some(_)) => return Err(bad(&format!("{at}.options"), Problem::Unknown)),
        (_, None) => Vec::new(),
    };
    for key in ["min", "max", "integer"] {
        match (control, object.get(key)) {
            (_, None) => {}
            ("number", Some(Value::Number(_))) if key != "integer" => {}
            ("number", Some(Value::Bool(_))) if key == "integer" => {}
            ("number", Some(_)) => return Err(bad(&format!("{at}.{key}"), Problem::Type)),
            (_, Some(_)) => return Err(bad(&format!("{at}.{key}"), Problem::Unknown)),
        }
    }
    let value_at = format!("{at}.value");
    let Some(value) = object.get("value") else {
        return Err(bad(&value_at, Problem::Missing));
    };
    value_of(&value_at, control, value, &options)
}

/// 值照控件的类型：开关是布尔，数字是数，下拉是选项里的一个，字是一行（多行的能换行），列表是字的数组；`null` 都收。
fn value_of(at: &str, control: &str, value: &Value, options: &[&str]) -> Result<(), Bad> {
    let text = |text: &str, most: usize, lines: bool| {
        if text.chars().count() > most {
            Err(bad(at, Problem::TooLong))
        } else if !clean(text, lines) {
            Err(bad(at, Problem::ControlChar))
        } else {
            Ok(())
        }
    };
    match (control, value) {
        (_, Value::Null) | ("toggle", Value::Bool(_)) | ("number", Value::Number(_)) => Ok(()),
        ("select", Value::String(chosen)) if options.contains(&chosen.as_str()) => Ok(()),
        ("text", Value::String(written)) => text(written, SHORT, false),
        ("textarea", Value::String(written)) => text(written, LONG, true),
        ("list", Value::Array(list)) if list.len() > MOST_OPTIONS => Err(bad(at, Problem::TooMany)),
        ("list", Value::Array(list)) => {
            for one in list {
                let Value::String(one) = one else {
                    return Err(bad(at, Problem::Value));
                };
                text(one, SHORT, false)?;
            }
            Ok(())
        }
        _ => Err(bad(at, Problem::Value)),
    }
}

/// 下拉的选项：`[{"value": <字符串>, "label": <字>}]`，值不重。交回几个值。
fn options<'a>(at: &str, list: &'a Value) -> Result<Vec<&'a str>, Bad> {
    let list = array(at, Some(list), MOST_OPTIONS)?;
    let mut values = Vec::new();
    for (n, option) in list.iter().enumerate() {
        let at = format!("{at}[{n}]");
        let Some(option) = option.as_object() else {
            return Err(bad(&at, Problem::Type));
        };
        only(&at, option, &["value", "label"])?;
        required(&at, option, "label", SHORT, false)?;
        let value_at = format!("{at}.value");
        let value = match option.get("value") {
            Some(Value::String(value)) => value.as_str(),
            Some(_) => return Err(bad(&value_at, Problem::Type)),
            None => return Err(bad(&value_at, Problem::Missing)),
        };
        if value.chars().count() > SHORT || !clean(value, false) {
            return Err(bad(&value_at, Problem::TooLong));
        }
        if values.contains(&value) {
            return Err(bad(&value_at, Problem::Duplicate));
        }
        values.push(value);
    }
    Ok(values)
}

/// 「加一项」：标签、调哪个方法，可以带下拉的选项。
fn add_one(at: &str, add: &Value) -> Result<(), Bad> {
    let Some(add) = add.as_object() else {
        return Err(bad(at, Problem::Type));
    };
    only(at, add, &["label", "call", "options"])?;
    required(at, add, "label", SHORT, false)?;
    call(at, add, false)?;
    match add.get("options") {
        Some(list) => options(&format!("{at}.options"), list).map(|_| ()),
        None => Ok(()),
    }
}

/// `call`：程序登记的方法名的写法；`params` 能写的（保存）只能是对象。
fn call(at: &str, object: &Map<String, Value>, with_params: bool) -> Result<(), Bad> {
    let call_at = format!("{at}.call");
    match object.get("call") {
        Some(Value::String(method)) if method_name(method) => {}
        Some(_) => return Err(bad(&call_at, Problem::Action)),
        None => return Err(bad(&call_at, Problem::Missing)),
    }
    match object.get("params") {
        Some(params) if with_params && !params.is_object() => {
            Err(bad(&format!("{at}.params"), Problem::Type))
        }
        _ => Ok(()),
    }
}

/// 角标：一小段字（或语言表），或者 `{"text", "tone"?}`。
fn badge(at: &str, value: Option<&Value>) -> Result<(), Bad> {
    match value {
        None => Ok(()),
        Some(Value::Object(spans)) if spans.contains_key("text") => {
            only(at, spans, &["text", "tone"])?;
            required(at, spans, "text", BADGE, false)?;
            one_of(at, spans, "tone", &TONES, Problem::Tone)
        }
        Some(value) => words(value, at, BADGE, false),
    }
}

/// 编号、键：一段不换行的字（不是语言表），1 到 [`ID`] 个字，在 `seen` 里不重。
fn id(
    at: &str,
    object: &Map<String, Value>,
    key: &str,
    seen: &mut BTreeSet<String>,
) -> Result<(), Bad> {
    let at = format!("{at}.{key}");
    let id = match object.get(key) {
        Some(Value::String(id)) => id,
        Some(_) => return Err(bad(&at, Problem::Type)),
        None => return Err(bad(&at, Problem::Missing)),
    };
    if id.is_empty() || id.chars().count() > ID || !clean(id, false) {
        return Err(bad(&at, Problem::TooLong));
    }
    if !seen.insert(id.clone()) {
        return Err(bad(&at, Problem::Duplicate));
    }
    Ok(())
}

/// 必写的数组，最多 `most` 个。
fn array<'a>(at: &str, value: Option<&'a Value>, most: usize) -> Result<&'a Vec<Value>, Bad> {
    match value {
        Some(Value::Array(list)) if list.len() > most => Err(bad(at, Problem::TooMany)),
        Some(Value::Array(list)) => Ok(list),
        Some(_) => Err(bad(at, Problem::Type)),
        None => Err(bad(at, Problem::Missing)),
    }
}

/// 挑语言（[`super::localize`]）：一项的副标题、角标，表单每一格的字、选项，保存和「加一项」的标签；一项里面的段照样挑。
pub(super) fn pick_inside(object: &mut Map<String, Value>, language: &str) {
    use super::{localize_part, pick, pick_in};
    if let Some(items) = object.get_mut("items").and_then(Value::as_array_mut) {
        for item in items.iter_mut().filter_map(Value::as_object_mut) {
            pick_in(item, &["subtitle"], language);
            match item.get_mut("badge") {
                Some(Value::Object(spans)) if spans.contains_key("text") => {
                    pick_in(spans, &["text"], language);
                }
                Some(badge) => *badge = pick(badge, language),
                None => {}
            }
            if let Some(blocks) = item.get_mut("blocks").and_then(Value::as_array_mut) {
                for block in blocks.iter_mut() {
                    localize_part(block, language);
                }
            }
        }
    }
    if let Some(fields) = object.get_mut("fields").and_then(Value::as_array_mut) {
        for one in fields.iter_mut().filter_map(Value::as_object_mut) {
            pick_in(one, &["label", "hint", "placeholder"], language);
            if let Some(default) = one.get_mut("default").and_then(Value::as_object_mut) {
                pick_in(default, &["text"], language);
            }
            pick_options(one, language);
        }
    }
    for key in ["submit", "add"] {
        if let Some(inner) = object.get_mut(key).and_then(Value::as_object_mut) {
            pick_in(inner, &["label"], language);
            pick_options(inner, language);
        }
    }
}

/// 选项的标签照语言挑。
fn pick_options(object: &mut Map<String, Value>, language: &str) {
    if let Some(options) = object.get_mut("options").and_then(Value::as_array_mut) {
        for option in options.iter_mut().filter_map(Value::as_object_mut) {
            super::pick_in(option, &["label"], language);
        }
    }
}

#[cfg(test)]
mod tests;
