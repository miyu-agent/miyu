//! 资源（施工 X-41，`docs/blueprint/mcp.md`「怎么走」第七条）：`resources/list`、`resources/templates/list` 的一页，只留规范里
//! 那几格；`resources/read` 的内容换成 [`Content::Resource`]。

use serde_json::{Map, Value};

use crate::content::Content;

/// 一页：留下的几项，和下一页的 `cursor`（没有下一页的没有）。
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    /// 留下的几项：资源每项只有 [`RESOURCE_FIELDS`]，模板每项只有 [`TEMPLATE_FIELDS`]。
    pub items: Vec<Value>,
    /// 下一页的 `cursor`。
    pub next: Option<String>,
}

/// 资源一项留的几格：第一格（地址）是必有的。
pub const RESOURCE_FIELDS: [&str; 6] = ["uri", "name", "title", "description", "mimeType", "size"];

/// 模板一项留的几格：第一格（地址模板）是必有的。
pub const TEMPLATE_FIELDS: [&str; 5] = ["uriTemplate", "name", "title", "description", "mimeType"];

/// 回应 `result` 里 `key` 那一列的一页：每项只留 `fields`；`key` 不是数组的没有（写法不对）。
pub(crate) fn page(result: &Value, key: &str, fields: &[&str]) -> Option<Page> {
    let items = result
        .get(key)?
        .as_array()?
        .iter()
        .filter_map(|raw| kept(raw, fields))
        .collect();
    let next = result
        .get("nextCursor")
        .and_then(Value::as_str)
        .filter(|cursor| !cursor.is_empty())
        .map(str::to_string);
    Some(Page { items, next })
}

/// 一项只留 `fields` 这几格；不是对象的、第一格（地址、地址模板）不是字的不收。
fn kept(raw: &Value, fields: &[&str]) -> Option<Value> {
    let object = raw.as_object()?;
    object.get(*fields.first()?)?.as_str()?;
    let kept: Map<String, Value> = fields
        .iter()
        .filter_map(|field| Some(((*field).to_string(), object.get(*field)?.clone())))
        .collect();
    Some(Value::Object(kept))
}

/// `resources/read` 回应里的内容：每一段换成 [`Content::Resource`]，没有地址的不收；`contents` 不是数组的没有（写法不对）。
pub(crate) fn contents(result: &Value) -> Option<Vec<Content>> {
    let text = |item: &Value, key: &str| item.get(key).and_then(Value::as_str).map(str::to_string);
    let contents = result
        .get("contents")?
        .as_array()?
        .iter()
        .filter_map(|item| {
            Some(Content::Resource {
                uri: text(item, "uri")?,
                mime: text(item, "mimeType"),
                text: text(item, "text"),
                blob: text(item, "blob"),
            })
        })
        .collect();
    Some(contents)
}
