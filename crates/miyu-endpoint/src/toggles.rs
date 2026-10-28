//! 技能、脚本、MCP 三页上改的（施工 X-27，网页设置页要的）：全局开关 `feature.set`（`features.<功能编号>`，
//! `miyu_session::switches`），只改管理员的个人设置，写法照 `config.set` 改一项（`config/set.rs`：带版本、不覆盖同时的手改、
//! 写成了推 `config.changed`）；交 `null` 的删掉那一项，回到跟着预设。下一个回合生效。只给本机的人用（`connection/people.rs`）。
//!
//! 加载方式 `tool.set_loading`（施工 L-3）施工 FL-1 起什么都不改：工具一律全量。协议 1 冻结了这个方法，照旧收、照旧查参数，
//! 回 `{}`；头的加载开关去掉了。

use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};

use miyu_config::key::{self, ID, NAME};
use miyu_kernel::id::CommandId;
use miyu_policy::preset::switchable;

use crate::Core;
use crate::config::set::{SetParams, set};
use crate::hello::Peer;
use crate::refusal::Refusal;

/// `tool.set_loading` 的参数：`tool`、`server` 正好写一个。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LoadingParams {
    /// 工具名。
    #[serde(default)]
    tool: Option<String>,
    /// MCP 服务名。
    #[serde(default)]
    server: Option<String>,
    /// 要不要先加载；`null` 是删掉。必写（退役了也照旧要写，老的头写错了照旧知道）。
    #[serde(deserialize_with = "present")]
    #[expect(dead_code, reason = "协议 1 冻结的参数：照旧要写，不再用")]
    need_load: Option<bool>,
}

/// `feature.set` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FeatureParams {
    /// 功能编号：`skill-`、`script-`、`mcp-` 开头的。
    feature: String,
    /// 开不开；`null` 是删掉。必写。
    #[serde(deserialize_with = "present")]
    on: Option<bool>,
}

/// 写成 `null` 的也算写了：没写的交给 serde 报缺了这一格。
fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<bool>, D::Error> {
    Option::<bool>::deserialize(d)
}

/// `tool.set_loading`：退役（施工 FL-1，工具一律全量），什么都不改，回应 `{}`。
///
/// # Errors
///
/// `tool`、`server` 不是正好一个，名字写法不对（工具名照 `<name>`、服务名照 `<id>`）的 `bad_params`：老的头照旧知道写错了。
pub(crate) fn set_loading(params: &LoadingParams) -> Result<Value, Refusal> {
    match (&params.tool, &params.server) {
        (Some(tool), None) if key::valid(NAME, tool) => Ok(json!({})),
        (None, Some(server)) if key::valid(ID, server) => Ok(json!({})),
        _ => Err(Refusal::BAD_PARAMS),
    }
}

/// `feature.set`：改个人设置的 `features.<功能编号>`，回应 `{}`。
///
/// # Errors
///
/// 不是技能、脚本、MCP 服务的功能编号，写不进键（照 `<name>`）的 `bad_params`；别的同 `config.set`。
pub(crate) fn set_feature(
    core: &Core,
    peer: Peer,
    cause: &CommandId,
    params: FeatureParams,
) -> Result<Value, Refusal> {
    if !switchable(&params.feature) || !key::valid(NAME, &params.feature) {
        return Err(Refusal::BAD_PARAMS);
    }
    let real = key::fill(miyu_session::switches::FEATURE, &[&params.feature]);
    written(core, peer, cause, real, params.on)
}

/// 照 `config.set` 改个人设置里的一项 `real`：`value` 是 `None` 的删掉。
fn written(
    core: &Core,
    peer: Peer,
    cause: &CommandId,
    real: String,
    value: Option<bool>,
) -> Result<Value, Refusal> {
    set(core, peer, cause, SetParams::switch(real, value))?;
    Ok(json!({}))
}
