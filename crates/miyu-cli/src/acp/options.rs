//! 模式和会话配置项（设计 73 第五节第 5 条，`internal/blueprint/cli/acp.md`「模式和配置项」）：模式是 Miyu 的两档权限，开放
//! 权限（`full`）和只读（`read-only`）；配置项是模式、模型、思考档位三项，照规范两套都给（`modes` 给只认老写法的编辑器）。
//! 能选的模型照 `model.list`：有密钥的模型按供应商分组，加上能用的模型池；思考档位照会话这时的模型的档位，池、不带思考的没有。

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::session::Current;
use crate::language::Language;

/// 开放权限。
pub(crate) const FULL: &str = "full";
/// 只读。
pub(crate) const READ_ONLY: &str = "read-only";

/// 能选的模型：照核心的 `model.list`，开会话时、列表变了时重问（`choosing.rs`）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Choices {
    /// 分组：组名（供应商的名字、「模型池」）到这一组的 `(引用, 名字)`，照先后。
    groups: Vec<(String, Vec<(String, String)>)>,
    /// 模型的引用到它的思考档位。
    levels: BTreeMap<String, Vec<String>>,
}

impl Choices {
    /// 照 `model.list` 的回应读：有密钥的模型（`state` 不是 `no_key`），能用的池。
    pub(crate) fn of(listed: &Value, language: Language) -> Choices {
        let mut choices = Choices::default();
        let empty = Vec::new();
        for provider in listed["providers"].as_array().unwrap_or(&empty) {
            let name = provider["name"]["value"]
                .as_str()
                .or_else(|| provider["id"].as_str())
                .unwrap_or_default();
            let mut group = Vec::new();
            for model in provider["models"].as_array().unwrap_or(&empty) {
                let (Some(reference), Some(id)) = (model["ref"].as_str(), model["model"].as_str())
                else {
                    continue;
                };
                if model["state"] == "no_key" || model["embedding"] == true {
                    continue;
                }
                let shown = model["facts"]["name"]["value"].as_str().unwrap_or(id);
                group.push((reference.to_string(), shown.to_string()));
                let levels: Vec<String> =
                    serde_json::from_value(model["facts"]["reasoning"]["value"].clone())
                        .unwrap_or_default();
                if !levels.is_empty() {
                    choices.levels.insert(reference.to_string(), levels);
                }
            }
            if !group.is_empty() {
                choices.groups.push((name.to_string(), group));
            }
        }
        let pools: Vec<(String, String)> = listed["pools"]
            .as_array()
            .unwrap_or(&empty)
            .iter()
            .filter(|pool| pool["usable"] != false)
            .filter_map(|pool| pool["name"].as_str())
            .map(|name| (format!("@{name}"), format!("@{name}")))
            .collect();
        if !pools.is_empty() {
            choices
                .groups
                .push((language.acp_pools().to_string(), pools));
        }
        choices
    }

    /// 有没有这个模型可选。
    pub(crate) fn offers(&self, reference: &str) -> bool {
        self.groups
            .iter()
            .any(|(_, group)| group.iter().any(|(value, _)| value == reference))
    }

    /// 这个模型的思考档位；池、不带思考的是空的。
    pub(crate) fn levels(&self, reference: Option<&str>) -> &[String] {
        reference
            .and_then(|reference| self.levels.get(reference))
            .map_or(&[], Vec::as_slice)
    }
}

/// 老写法的模式（`modes`）：两档权限，当前是哪一档。
pub(crate) fn modes(language: Language, current: &Current) -> Value {
    let available: Vec<Value> = [FULL, READ_ONLY]
        .into_iter()
        .map(|id| json!({"id": id, "name": language.acp_mode(id)}))
        .collect();
    json!({"currentModeId": current.mode, "availableModes": available})
}

/// 会话配置项（`configOptions`），照规范的先后：模式、模型、思考档位。模型一个能选的都没有的不给模型那一项；现在用的不在
/// 能选的里面的（配置改过、钉着的没了）也列上，不然编辑器显示不了它。不带思考的不给思考那一项。
pub(crate) fn options(language: Language, choices: &Choices, current: &Current) -> Vec<Value> {
    let modes: Vec<Value> = [FULL, READ_ONLY]
        .into_iter()
        .map(|id| json!({"value": id, "name": language.acp_mode(id)}))
        .collect();
    let mut out = vec![json!({
        "id": "mode", "name": language.acp_option("mode"), "category": "mode",
        "type": "select", "currentValue": current.mode, "options": modes,
    })];
    if let Some(model) = &current.model {
        let mut groups: Vec<Value> = choices
            .groups
            .iter()
            .map(|(name, group)| {
                let options: Vec<Value> = group
                    .iter()
                    .map(|(value, shown)| json!({"value": value, "name": shown}))
                    .collect();
                json!({"group": name, "name": name, "options": options})
            })
            .collect();
        if !choices.offers(model) {
            let only = json!([{"value": model, "name": model}]);
            groups.insert(0, json!({"group": model, "name": model, "options": only}));
        }
        out.push(json!({
            "id": "model", "name": language.acp_option("model"), "category": "model",
            "type": "select", "currentValue": model, "options": groups,
        }));
    }
    let levels = choices.levels(current.model.as_deref());
    if let (Some(effort), false) = (&current.effort, levels.is_empty()) {
        let options: Vec<Value> = levels
            .iter()
            .map(|level| json!({"value": level, "name": level}))
            .collect();
        out.push(json!({
            "id": "effort", "name": language.acp_option("effort"), "category": "thought_level",
            "type": "select", "currentValue": effort, "options": options,
        }));
    }
    out
}

/// 编辑器要换的一项写成给核心的请求：方法、参数；认不得的项、值交回 `None`（照规范回 `-32602`）。
pub(crate) fn request(
    session: &str,
    choices: &Choices,
    current: &Current,
    id: &str,
    value: &str,
) -> Option<(&'static str, Value)> {
    match id {
        "mode" => permission(session, value),
        "model" if choices.offers(value) || current.model.as_deref() == Some(value) => Some((
            "session.configure",
            json!({"session": session, "model": value}),
        )),
        "effort"
            if choices
                .levels(current.model.as_deref())
                .iter()
                .any(|level| level == value) =>
        {
            Some((
                "session.configure",
                json!({"session": session, "effort": value}),
            ))
        }
        _ => None,
    }
}

/// 切到模式 `mode`：`session.set_permission_level`；认不得的模式交回 `None`。
pub(crate) fn permission(session: &str, mode: &str) -> Option<(&'static str, Value)> {
    let params = match mode {
        FULL => json!({"session": session, "level": "full", "read_only": false}),
        READ_ONLY => json!({"session": session, "read_only": true}),
        _ => return None,
    };
    Some(("session.set_permission_level", params))
}

/// 换好以后的样子：照换了的那一项改 `current`（核心推来的会话状态随后会再对一遍）。
pub(crate) fn applied(current: &Current, id: &str, value: &str) -> Current {
    let mut after = current.clone();
    match id {
        "mode" => after.mode = value.to_string(),
        "model" => after.model = Some(value.to_string()),
        "effort" => after.effort = Some(value.to_string()),
        _ => {}
    }
    after
}

#[cfg(test)]
mod tests;
