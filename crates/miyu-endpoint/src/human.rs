//! `human.get`（给人看的字，施工 W-1，`docs/blueprint/web-module.md`「二、给人看的字」）：工具的样子、说法的模板，
//! 核心经协议交出去，终端、网页都不用再自己去资源目录里读。
//!
//! 每次现读资源目录（`web-module.md`「起草时定的」第 32 条）：一个连接只要一次，读几份 JSON 很快；开发时改了资源，
//! 下一次调就是新的，不用重启核心。在阻塞线程里读。工具目录里这一刻写了显示名的脚本、MCP 的、扩展的工具也并进 `tools`（施工
//! 9-8 七补）。每件工具属于哪个包另给一张表 `packages`（施工 X-24 补）。

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use miyu_store::human::Human;

use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

const TARGET: &str = "miyu::endpoint";

/// `human.get` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct Params {
    /// 要哪种语言：2 到 8 个小写字母，例如 `zh`。不写照这个连接的语言。
    #[serde(default)]
    language: Option<String>,
}

/// `human.get`：给人看的字，工具的样子、说法的模板原文。回应里没有 `config` 那一格：配置的名字、说明在
/// `config.schema` 里（施工 8-2）。
pub(crate) async fn get(core: &Core, peer: Peer, params: Params) -> Result<Value, Refusal> {
    let language = params.language.unwrap_or_else(|| peer.language.to_string());
    if !is_language_code(&language) {
        return Err(Refusal::BAD_PARAMS);
    }
    let resources = core.resources.clone();
    let wanted = language.clone();
    let loaded = tokio::task::spawn_blocking(move || Human::load(&resources, &wanted)).await;
    let human = match loaded {
        Ok(Ok(human)) => human,
        Ok(Err(error)) => {
            tracing::warn!(target: TARGET, error = %error, "human not read");
            return Err(Refusal::INTERNAL);
        }
        Err(error) => {
            tracing::error!(target: TARGET, error = %error, "human panicked");
            return Err(Refusal::INTERNAL);
        }
    };
    let mut tools: Map<String, Value> = human
        .tools()
        .iter()
        .map(|(name, face)| (name.clone(), json!(face)))
        .collect();
    named(core, &language, &mut tools);
    let said: BTreeMap<&str, &str> = human.said_entries().collect();
    Ok(json!({"language": language, "said": said, "tools": tools, "packages": owned(core)}))
}

/// 工具目录里这一刻写了显示名的工具（脚本、MCP 的、扩展的，施工 9-8 七补）并进 `tools`：照 `language` 挑（这种语言没写的照
/// 英文、中文、日文的先后），`{"name": …}`；扩展登记时写了跟在名字后面的参数的，多一格 `subject`（施工 S-6 上），只写了它、
/// 没写显示名的，名字是工具名。上面读出来的字里已经有的照那一份。
fn named(core: &Core, language: &str, tools: &mut Map<String, Value>) {
    let catalog = core.tools();
    for spec in catalog.specs() {
        if tools.contains_key(&spec.name) {
            continue;
        }
        let Some(tool) = catalog.get(&spec.name) else {
            continue;
        };
        let picked = crate::personas::pick(&tool.display_name(), language);
        let face = match (picked, tool.subject()) {
            (Some(name), None) => json!({ "name": name }),
            (picked, Some(subject)) => {
                json!({ "name": picked.unwrap_or_else(|| spec.name.clone()), "subject": subject })
            }
            (None, None) => continue,
        };
        tools.insert(spec.name.clone(), face);
    }
}

/// 工具目录里这一刻的每件工具属于哪个包（施工 X-24 补：终端照它判「只为发一张图」那一段）：工具名到包编号，单独一张表。
/// 不往 `tools` 的每一格里塞：头照 `Face` 读那一格、不认识的格子整格不收（X-24 塞进去时现有的终端读不出工具的样子）。
fn owned(core: &Core) -> BTreeMap<String, String> {
    let catalog = core.tools();
    catalog
        .specs()
        .filter_map(|spec| {
            Some((
                spec.name.clone(),
                catalog.package_of(&spec.name)?.to_string(),
            ))
        })
        .collect()
}

/// `language` 合不合写法：2 到 8 个小写字母（`protocol.md`「`human.get`」）。
fn is_language_code(language: &str) -> bool {
    (2..=8).contains(&language.len()) && language.bytes().all(|b| b.is_ascii_lowercase())
}
