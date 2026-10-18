//! 给头看的结构化结果的几种通用形状（施工 W-11 补，2026-10-18 项目主人定）：扩展的工具、以后的内置工具都照这几种写，头认得
//! 的画成卡片，别的不画。内置工具原有的形状（`memory_search`、`kb_search`、`kb_read`）不变，头照工具名认。
//!
//! - `links`：一串链接，标题、网址、摘要、站名、日期。网址只收 `http`、`https`。
//! - `table`：一张表，几列的列名、几行，一行的格数和列数一样，格子是字、数、真假或者空。
//! - `images`：几张图，引用 blob（先 `blob.put`），媒体类型是 `image/` 开头的。
//!
//! [`read`] 照形状读别人交来的一份：多一格、少一格、超了上限的整份不要，交回原因（记运行日志用）。

use serde::{Deserialize, Serialize};
use serde_json::Value;

use miyu_kernel::id::{ContentHash, MediaType};

/// 一串链接至多几条。
pub const MOST_LINKS: usize = 50;

/// 一张表至多几列。
pub const MOST_COLUMNS: usize = 20;

/// 一张表至多几行。
pub const MOST_ROWS: usize = 200;

/// 至多几张图。
pub const MOST_IMAGES: usize = 20;

/// 整份写成 JSON 至多几个字节。
pub const MOST_BYTES: usize = 64 * 1024;

/// 一份给头看的结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum Detail {
    /// 一串链接。
    Links {
        /// 照给的先后。
        links: Vec<Link>,
    },
    /// 一张表。
    Table {
        /// 列名。
        columns: Vec<String>,
        /// 几行，一行的格数和列数一样。
        rows: Vec<Vec<Value>>,
    },
    /// 几张图。
    Images {
        /// 照给的先后。
        images: Vec<ImageRef>,
    },
}

/// 一条链接。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    /// 标题。
    pub title: String,
    /// 网址：`http`、`https`。
    pub url: String,
    /// 一段摘要。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    /// 站名。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    /// 日期，照来源给的原样写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

/// 一张图。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageRef {
    /// 存成的 blob。
    pub blob: ContentHash,
    /// `image/` 开头的媒体类型。
    pub media_type: MediaType,
    /// 图下面的一句。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
}

impl Detail {
    /// 写成 JSON。
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    /// 引用的 blob：只有图有。
    pub fn blobs(&self) -> Vec<ContentHash> {
        match self {
            Detail::Images { images } => images.iter().map(|image| image.blob.clone()).collect(),
            Detail::Links { .. } | Detail::Table { .. } => Vec::new(),
        }
    }

    /// 照上限和写法查一遍：查不过的交回原因。
    fn check(&self) -> Result<(), String> {
        match self {
            Detail::Links { links } => {
                if links.len() > MOST_LINKS {
                    return Err(format!("more than {MOST_LINKS} links"));
                }
                if let Some(link) = links.iter().find(|link| !web(&link.url)) {
                    return Err(format!("not an http or https url: {}", link.url));
                }
            }
            Detail::Table { columns, rows } => {
                if columns.is_empty() || columns.len() > MOST_COLUMNS {
                    return Err(format!("columns must be 1 to {MOST_COLUMNS}"));
                }
                if rows.len() > MOST_ROWS {
                    return Err(format!("more than {MOST_ROWS} rows"));
                }
                if rows.iter().any(|row| row.len() != columns.len()) {
                    return Err("a row does not have one cell per column".to_string());
                }
                let cell = |value: &Value| !matches!(value, Value::Array(_) | Value::Object(_));
                if !rows.iter().flatten().all(cell) {
                    return Err("a cell is not a string, number, boolean or null".to_string());
                }
            }
            Detail::Images { images } => {
                if images.len() > MOST_IMAGES {
                    return Err(format!("more than {MOST_IMAGES} images"));
                }
                if let Some(image) = images
                    .iter()
                    .find(|image| !image.media_type.as_str().starts_with("image/"))
                {
                    return Err(format!("not an image: {}", image.media_type.as_str()));
                }
            }
        }
        Ok(())
    }
}

/// 照形状读一份：读不成、查不过、写成 JSON 超过 [`MOST_BYTES`] 的交回原因。
///
/// # Errors
///
/// 原因是一句英文，记运行日志用。
pub fn read(value: &Value) -> Result<Detail, String> {
    let size = value.to_string().len();
    if size > MOST_BYTES {
        return Err(format!("{size} bytes, more than {MOST_BYTES}"));
    }
    let detail: Detail =
        serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
    detail.check()?;
    Ok(detail)
}

/// `http://`、`https://` 开头、后面还有字（不分大小写）。
fn web(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    ["http://", "https://"]
        .iter()
        .any(|scheme| lower.starts_with(scheme) && lower.len() > scheme.len())
}

#[cfg(test)]
mod tests;
