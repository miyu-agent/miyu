//! 供应商的额度（施工 QU-1，`docs/blueprint/models.md`「怎么走」第十七条）：有的供应商另有一个查额度的接口，回这一刻几个窗口
//! 各用了百分之几、什么时候重置。档案（`profiles.toml`）的 `quota` 一格写地址和回应的格式，配置的 `[providers.<id>.quota]`
//! 能改地址；代码里不写地址。
//!
//! 这一层只管纯的几样：档案的写法、和配置合起来、读回应。去查、隔多久查一次在 `miyu-session` 的 `route/quota.rs`。
//!
//! 格式只认 `opencode-go` 一种：`{"usage": {"rolling": {...}, "weekly": {...}, "monthly": {...}}}`，每个窗口
//! `{"status", "percent", "resetsAt"}`（2026-11-14 实测，官方文档没写）；`resetsAt` 照 RFC 3339 的各种写法读、换成 UTC
//! （`quota/instant.rs`）。以后别的供应商有额度接口，[`QuotaFormat`] 多一种、
//! [`read`] 多一支。

use miyu_config::Values;
use miyu_kernel::time::Timestamp;
use serde::Deserialize;
use serde_json::{Number, Value};

use crate::settings::QuotaSettings;

/// 查额度时另配的头（`x-opencode-session` 这类）照的种子：额度不属于哪个会话，同一家共用一份，和一次性调用照用途一样固定一个。
pub const SEED: &str = "quota";

/// 档案里 `quota` 那一格：地址和回应的格式。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quota {
    /// 完整的地址，GET 它。
    pub url: String,
    /// 回应照哪一种读。
    pub format: QuotaFormat,
}

/// 回应的格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QuotaFormat {
    /// opencode Go：`Authorization: Bearer <key>`，三个窗口。
    OpencodeGo,
}

/// 一个窗口。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    /// 哪一个。
    pub id: WindowId,
    /// 用了百分之几，原样。
    pub percent: Number,
    /// 供应商说的状态，原样（实测见过 `ok`）。
    pub status: String,
    /// 什么时候重置。
    pub resets_at: Timestamp,
}

/// 窗口的编号，协议里照它写，头照它认。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WindowId {
    /// 5 小时滚动的。
    Rolling,
    /// 每周。
    Weekly,
    /// 每月。
    Monthly,
}

impl WindowId {
    /// 照这个先后交。
    pub const ALL: [WindowId; 3] = [WindowId::Rolling, WindowId::Weekly, WindowId::Monthly];

    /// 协议里的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            WindowId::Rolling => "rolling",
            WindowId::Weekly => "weekly",
            WindowId::Monthly => "monthly",
        }
    }
}

/// 编号 `id` 这一家查额度照哪个：档案写了才有（格式只有档案知道），配置写了 `url` 的盖过档案的地址。
pub fn settled(profile: Option<&Quota>, values: &Values, id: &str) -> Option<Quota> {
    let mut quota = profile?.clone();
    if let Some(url) = QuotaSettings::at(values, &[id])
        .url
        .filter(|url| !url.trim().is_empty())
    {
        quota.url = url;
    }
    Some(quota)
}

/// 照格式 `format` 读回应 `bytes`：照 [`WindowId::ALL`] 的先后，回应里没有的窗口不交。
///
/// # Errors
///
/// 不是 JSON、一个窗口都没有、有的窗口写法不对（缺格、类型不对、时刻读不出）：英文的一句。
pub fn read(format: QuotaFormat, bytes: &[u8]) -> Result<Vec<Window>, String> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|error| format!("not JSON: {error}"))?;
    let windows = match format {
        QuotaFormat::OpencodeGo => opencode_go(&value)?,
    };
    match windows.is_empty() {
        true => Err("no window in the reply".to_string()),
        false => Ok(windows),
    }
}

/// opencode Go 的写法。
fn opencode_go(value: &Value) -> Result<Vec<Window>, String> {
    let usage = value
        .get("usage")
        .and_then(Value::as_object)
        .ok_or("no usage object")?;
    let mut windows = Vec::new();
    for id in WindowId::ALL {
        let Some(window) = usage.get(id.as_str()) else {
            continue;
        };
        let bad = |what: &str| format!("{} has no usable {what}", id.as_str());
        let percent = match window.get("percent") {
            Some(Value::Number(number)) => number.clone(),
            _ => return Err(bad("percent")),
        };
        let status = window
            .get("status")
            .and_then(Value::as_str)
            .ok_or_else(|| bad("status"))?;
        let resets_at = window
            .get("resetsAt")
            .and_then(Value::as_str)
            .and_then(instant::rfc3339)
            .ok_or_else(|| bad("resetsAt"))?;
        windows.push(Window {
            id,
            percent,
            status: status.to_string(),
            resets_at,
        });
    }
    Ok(windows)
}

mod instant;
#[cfg(test)]
mod tests;
