//! 供应商的额度（施工 QU-1，`internal/blueprint/models.md`「怎么走」第十七条）：有的供应商另有一个查额度的接口，回这一刻几个窗口
//! 各用了百分之几、什么时候重置，或者账户里还有多少钱（施工 QU-2）。档案（`profiles.toml`）的 `quota` 一格写地址和回应的格式，
//! 配置的 `[providers.<id>.quota]` 能改地址；代码里不写地址。
//!
//! 这一层只管纯的几样：档案的写法、和配置合起来、读回应。去查、隔多久查一次在 `miyu-session` 的 `route/quota.rs`。
//!
//! 格式两种：
//!
//! - `opencode-go`：`{"usage": {"rolling": {...}, "weekly": {...}, "monthly": {...}}}`，每个窗口
//!   `{"status", "percent", "resetsAt"}`（2026-11-14 实测，官方文档没写）；`resetsAt` 照 RFC 3339 的各种写法读、换成 UTC
//!   （`quota/instant.rs`）。
//! - `deepseek-balance`（施工 QU-2）：DeepSeek 官方的余额，读法在 `quota/balance.rs`。
//!
//! 目录里认得出官方地址的那一家（[`crate::provider_key::OFFICIAL`]，现在是 DeepSeek），供应商的地址还得是官方的才查：经中转站
//! 跑的、地址是环境变量引用的，key 不是这一家官方的，发过去是把别人的 key 交给它（同 PK-1 补）。配置改了查的地址也照这一条。
//! 以后别的供应商有额度接口，[`QuotaFormat`] 多一种、[`read`] 多一支。

use miyu_config::Values;
use miyu_kernel::time::Timestamp;
use serde::Deserialize;
use serde_json::{Number, Value};

use crate::provider_key::{official_address, official_host};
use crate::settings::{ProviderSettings, QuotaSettings};

pub use balance::{Balance, BalanceItem};

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
    /// DeepSeek 官方的余额（施工 QU-2）：`Authorization: Bearer <key>`，可不可用和每种币的余额。
    DeepseekBalance,
}

/// 查到的：窗口、余额各是各的，这一种格式没有的空着。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Reading {
    /// 几个窗口，照 [`WindowId::ALL`] 的先后；没有的是空的。
    pub windows: Vec<Window>,
    /// 余额；没有的是 `None`。
    pub balance: Option<Balance>,
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

/// 编号 `id` 这一家（照目录里 `catalog` 那一家的档案 `profile`）查额度照哪个：档案写了才有（格式只有档案知道），配置写了
/// `url` 的盖过档案的地址。认得出官方地址的那一家，配置里写的供应商地址不是官方的不查（[`official_address`]），改了查的
/// 地址也一样。
pub fn settled(profile: Option<&Quota>, values: &Values, id: &str, catalog: &str) -> Option<Quota> {
    let mut quota = profile?.clone();
    let written = ProviderSettings::at(values, &[id]).base_url;
    if official_host(catalog).is_some() && !official_address(written.as_ref(), catalog) {
        return None;
    }
    if let Some(url) = QuotaSettings::at(values, &[id])
        .url
        .filter(|url| !url.trim().is_empty())
    {
        quota.url = url;
    }
    Some(quota)
}

/// 照格式 `format` 读回应 `bytes`。窗口照 [`WindowId::ALL`] 的先后，回应里没有的窗口不交。
///
/// # Errors
///
/// 不是 JSON；窗口的格式一个窗口都没有、有的窗口写法不对（缺格、类型不对、时刻读不出）；余额的格式写法不对
/// （`quota/balance.rs`）：英文的一句。
pub fn read(format: QuotaFormat, bytes: &[u8]) -> Result<Reading, String> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|error| format!("not JSON: {error}"))?;
    match format {
        QuotaFormat::OpencodeGo => {
            let windows = opencode_go(&value)?;
            match windows.is_empty() {
                true => Err("no window in the reply".to_string()),
                false => Ok(Reading {
                    windows,
                    balance: None,
                }),
            }
        }
        QuotaFormat::DeepseekBalance => Ok(Reading {
            windows: Vec::new(),
            balance: Some(balance::deepseek(value)?),
        }),
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

mod balance;
mod instant;
#[cfg(test)]
mod tests;
