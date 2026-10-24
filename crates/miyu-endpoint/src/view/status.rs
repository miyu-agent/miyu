//! 会话状态（施工 9-8 补上，`docs/blueprint/view.md`「会话状态」）：投影算得出的那一半（在跑没跑、在等什么、正在做什么、
//! 上下文用了多少、速度、冷却、任务表），拼上向会话要的（用量、模型、权限、待办、工作区、长期目标）和订阅时定下的（人格、预设）。
//! 视图流订阅的回应带一份，之后变了推整份 `view.status`。

use serde_json::{Value, json};

use miyu_kernel::id::AccountId;
use miyu_session::{Current, Goal, Handle};
use miyu_view::Status;

use crate::Core;

/// 订阅时定下的：会话用哪个人格、哪个预设（照日志第一条 `session.created`，以前的日志没有的是 `None`）。向会话要得到「当前的」
/// 照那一份（中途换了的，施工 P-13）。
#[derive(Debug, Clone, Default)]
pub(crate) struct Fixed {
    pub(crate) persona: Option<Value>,
    pub(crate) preset: Option<Value>,
    /// 订阅时会话是明着无工具的（施工 P-16 下补）：`preset` 那时是 null。
    pub(crate) no_tools: bool,
}

/// 整份会话状态。`current` 是向会话 actor 要的那一份（拿不到的不写那几格）。
pub(crate) fn whole(
    core: &Core,
    handle: &Handle,
    projected: Status,
    current: Option<&Current>,
    fixed: &Fixed,
) -> Value {
    let used = projected.used;
    let mut status = json!(projected);
    if let Some(status) = status.as_object_mut() {
        status.remove("used");
    }
    let limits = handle.limits();
    let mut context = json!(limits);
    if let Some(used) = used {
        context["used"] = json!(used);
    }
    status["context"] = context;
    status["default_workspace"] = json!(handle.default_workspace().to_string_lossy());
    if let Some(model) = crate::models::next(&handle.next()) {
        status["model"] = model;
    }
    status["todos"] = json!(handle.todos());
    // 因为全部做完而清空的（施工 9-8 九补）：清空前那一份，同事件流 `todos.changed` 的 `done`；别的时候不写。
    let done = handle.todos_done();
    if !done.is_empty() {
        status["todos_done"] = json!(done);
    }
    if let Some(current) = current {
        status["usage"] = crate::usage::tallied(core, handle.owner(), &current.tally);
        status["permission"] = json!(current.permission);
        status["workspace"] = json!({"cwd": current.cwd, "dirs": current.dirs});
        // 有没有能压的（施工 W-12）：每落一批盘重取「当前的」，变了跟着推。
        status["compactable"] = json!(current.compactable);
        if let Some(goal) = &current.goal {
            status["goal"] = goal_of(core, handle.owner(), goal);
        }
    }
    // 中途换了的照向会话要的那一份（施工 P-13）；拿不到的照订阅时定下的。
    let persona = match current {
        Some(current) => current.persona.as_ref().map(|persona| json!(persona)),
        None => fixed.persona.clone(),
    };
    if let Some(persona) = persona {
        status["persona"] = persona;
    }
    // 预设同会话列表的写法（施工 P-16 下补）：无工具的（内部是空的编号）写 null 加 `no_tools: true`。
    match current.and_then(|current| current.preset.as_deref()) {
        Some(preset) => crate::presets::shown(&mut status, preset),
        None => {
            if let Some(preset) = &fixed.preset {
                status["preset"] = preset.clone();
            }
            if fixed.no_tools {
                status["no_tools"] = json!(true);
            }
        }
    }
    status
}

/// 会话状态的 `goal` 那一格（施工 X-6，`docs/blueprint/view.md`「会话状态」）：原文、上限（不限的不写）、状态、卡在哪（卡住的
/// 才有）、第几轮、设下的时刻、花了多少（写法同会话状态的 `usage`，币种照会话属主 `owner` 的）。
pub(crate) fn goal_of(core: &Core, owner: &AccountId, goal: &Goal) -> Value {
    let mut shown = json!({
        "text": goal.text,
        "state": goal.state.as_str(),
        "round": goal.round,
        "since": goal.since,
        "usage": crate::usage::tallied(core, owner, &goal.spent),
    });
    if let Some(limit) = goal.limit {
        shown["limit"] = json!(limit.get());
    }
    if let Some(reason) = &goal.reason {
        shown["reason"] = json!(reason);
    }
    shown
}

#[cfg(test)]
mod tests;
