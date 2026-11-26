//! 会话状态翻成 `session/update`（设计 73 第五节第 3、5 条，`internal/blueprint/cli/acp.md`「会话状态」）：待办 `plan`、
//! 上下文用量 `usage_update`、标题 `session_info_update`；权限、模型、思考档位变了交回 [`Action::Config`]，在等人的交回
//! [`Action::Waiting`]。和上一次报的一样的不再报。

use serde_json::{Value, json};

use super::options;
use super::session::{Current, Session, waiting_key};
use super::view::Action;

impl Session {
    /// 整份会话状态（`view.status`、订阅回应里的 `status`）。
    pub(crate) fn status(&mut self, status: &Value) -> Vec<Action> {
        let mut actions = Vec::new();
        let current = current(status, &self.current);
        if current != self.current {
            self.current = current;
            actions.push(Action::Config);
        }
        if let Some(plan) = plan(status)
            && self.reported.plan.as_ref() != Some(&plan)
        {
            // 从来没有待办的不报一份空的。
            let empty = plan["entries"].as_array().is_some_and(Vec::is_empty);
            if !(empty && self.reported.plan.is_none()) {
                actions.push(Action::Update(plan.clone()));
            }
            self.reported.plan = Some(plan);
        }
        if let Some(usage) = usage(status)
            && self.reported.usage.as_ref() != Some(&usage)
        {
            actions.push(Action::Update(usage.clone()));
            self.reported.usage = Some(usage);
        }
        if let Some(title) = status["title"].as_str()
            && self.reported.title.as_deref() != Some(title)
        {
            let update = json!({"sessionUpdate": "session_info_update", "title": title});
            actions.push(Action::Update(update));
            self.reported.title = Some(title.to_string());
        }
        let waiting: Vec<miyu_view::Waiting> =
            serde_json::from_value(status["waiting"].clone()).unwrap_or_default();
        // 已经了结的（在别的头上答了的、到时被拒的）忘掉：同一件再来要再问。
        let keys: Vec<String> = waiting.iter().map(waiting_key).collect();
        self.asked.retain(|key| keys.contains(key));
        if !waiting.is_empty() {
            actions.push(Action::Waiting(waiting));
        }
        actions
    }
}

/// 会话状态里的权限、模型、思考档位；没写的照 `before`。只读开着的是只读，别的是开放权限。
fn current(status: &Value, before: &Current) -> Current {
    let mode = match status["permission"]["read_only"].as_bool() {
        Some(true) => options::READ_ONLY.to_string(),
        Some(false) => options::FULL.to_string(),
        None => before.mode.clone(),
    };
    let model = &status["model"];
    match model.is_object() {
        true => Current {
            mode,
            model: model["ref"].as_str().map(str::to_string),
            effort: model["effort"]["level"].as_str().map(str::to_string),
        },
        false => Current {
            mode,
            ..before.clone()
        },
    }
}

/// 待办写成 `plan`：一项一条，规范要的轻重一律写 `medium`（Miyu 的待办没有轻重）。会话状态里没有 `todos` 的没有。
fn plan(status: &Value) -> Option<Value> {
    let todos = status["todos"].as_array()?;
    let entries: Vec<Value> = todos
        .iter()
        .filter_map(|todo| {
            let content = todo["content"].as_str()?;
            // 认不得的（以后的版本加的）照还没开始写：规范只有这三种。
            let state = todo["status"]
                .as_str()
                .filter(|state| matches!(*state, "pending" | "in_progress" | "completed"))
                .unwrap_or("pending");
            Some(json!({"content": content, "priority": "medium", "status": state}))
        })
        .collect();
    Some(json!({"sessionUpdate": "plan", "entries": entries}))
}

/// 上下文用量写成 `usage_update`：用了多少、窗口多大，两个都知道才报（规范要两个都在）。金额照 Miyu 算的：只有一种币的照
/// 原样，几种币的照换算好的那一个（有换算不了的那几笔就不报金额，算不准的钱不显示）。
fn usage(status: &Value) -> Option<Value> {
    let context = &status["context"];
    let (used, size) = (context["used"].as_u64()?, context["window"].as_u64()?);
    let mut update = json!({"sessionUpdate": "usage_update", "used": used, "size": size});
    if let Some(cost) = cost(&status["usage"]) {
        update["cost"] = cost;
    }
    Some(update)
}

/// 累计的金额（`{amount, currency}`）；算不出、算不准的没有。
fn cost(usage: &Value) -> Option<Value> {
    if usage["unpriced"].as_u64().unwrap_or(0) > 0 {
        return None;
    }
    if let Some([only]) = usage["amounts"].as_array().map(Vec::as_slice) {
        let (amount, currency) = (only["amount"].as_f64()?, only["currency"].as_str()?);
        return Some(json!({"amount": amount, "currency": currency}));
    }
    let converted = &usage["converted"];
    if converted.get("rest").is_some_and(|rest| !rest.is_null()) {
        return None;
    }
    let (amount, currency) = (
        converted["amount"].as_f64()?,
        converted["currency"].as_str()?,
    );
    Some(json!({"amount": amount, "currency": currency}))
}

#[cfg(test)]
mod tests;
