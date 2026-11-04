//! `session.export {format: "markdown"}` 的参数怎么读（施工 SE-1 中下中，调研第五节第 3 条）：`for` 定预设，写了的开关盖过
//! 预设。
//!
//! | 参数 | 收的 |
//! |---|---|
//! | `for` | `read`（不写的也是）、`agent`、`chat` |
//! | `range` | `all`、`context`、`{"turns": N}`（N ≥ 1） |
//! | `thinking` | 布尔 |
//! | `tools` | `line`、`seen`、`full` |
//! | `children` | `reports`；`append` 下一步才有 |
//! | `assets` | `placeholder`；`files` 下一步才有 |
//! | `locale` | `zh`、`ja` 开头的照它，别的一律英文；不写的照连接握手时的 |
//!
//! 下一步才有的拒 `bad_params`，`data.later` 写参数名；别的写法（写错的、类型不对的）照旧 `bad_params`、不带 `later`。

use std::path::PathBuf;

use serde_json::Value;

use miyu_view::markdown::{Options, Purpose, Range, Tools};

use super::{ExportParams, Inline, To, checked};
use crate::refusal::Refusal;

/// 写到哪。
pub(super) enum Target {
    /// 本机的一个文件。
    File(PathBuf),
    /// 放进回应。
    Inline,
}

/// 照参数定写到哪、用哪种语言（`language` 是连接握手时的）、哪一套开关。
pub(super) fn wanted(
    params: ExportParams,
    language: &'static str,
) -> Result<(Target, &'static str, Options), Refusal> {
    let purpose = match params.purpose.as_deref() {
        None | Some("read") => Purpose::Read,
        Some("agent") => Purpose::Agent,
        Some("chat") => Purpose::Chat,
        Some(_) => return Err(Refusal::BAD_PARAMS),
    };
    let mut options = Options::preset(purpose);
    if let Some(range) = &params.range {
        options.range = range_of(range)?;
    }
    match &params.thinking {
        None => {}
        Some(Value::Bool(thinking)) => options.thinking = *thinking,
        Some(_) => return Err(Refusal::BAD_PARAMS),
    }
    match params.tools.as_ref().map(Value::as_str) {
        None => {}
        Some(Some("line")) => options.tools = Tools::Line,
        Some(Some("seen")) => options.tools = Tools::Seen,
        Some(Some("full")) => options.tools = Tools::Full,
        Some(_) => return Err(Refusal::BAD_PARAMS),
    }
    for (name, switch, now, later) in [
        ("children", &params.children, "reports", "append"),
        ("assets", &params.assets, "placeholder", "files"),
    ] {
        match switch.as_ref().map(Value::as_str) {
            None => {}
            Some(Some(value)) if value == now => {}
            Some(Some(value)) if value == later => return Err(Refusal::later(name)),
            Some(_) => return Err(Refusal::BAD_PARAMS),
        }
    }
    let language = match params.locale.as_deref() {
        None => language,
        Some(locale) if locale.starts_with("zh") => "zh",
        Some(locale) if locale.starts_with("ja") => "ja",
        Some(_) => "en",
    };
    let target = match params.to {
        To::File(to) => Target::File(checked(to.path)?),
        To::Inline(Inline::Inline) => Target::Inline,
    };
    Ok((target, language, options))
}

/// `range` 的三种写法。
fn range_of(range: &Value) -> Result<Range, Refusal> {
    match range {
        Value::String(range) if range == "all" => Ok(Range::All),
        Value::String(range) if range == "context" => Ok(Range::Context),
        Value::Object(range) if range.len() == 1 => range
            .get("turns")
            .and_then(Value::as_u64)
            .filter(|n| *n > 0)
            .and_then(|n| u32::try_from(n).ok())
            .map(Range::Turns)
            .ok_or(Refusal::BAD_PARAMS),
        _ => Err(Refusal::BAD_PARAMS),
    }
}
