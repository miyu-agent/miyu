//! 权限策略判过的真实位置（施工 5-18，`session/tools.md` 第 1 条）：判的时候照调用编号记下，派的时候放进
//! [`miyu_tool::Call::checked`]，工具换出来的不在里面就不碰。问人的卡片上那一份看一眼（施工 PK-3 上）同样照调用编号记下，派的时候
//! 放进 [`miyu_tool::Call::previewed`]。嵌套调用的挂在它等着的那一份上（`nested.rs`）。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

use miyu_kernel::id::CallId;

use super::Tools;

/// 判过的：真实位置、问人的卡片上那一份看一眼，照调用编号。
#[derive(Default)]
pub(crate) struct Judged {
    paths: BTreeMap<CallId, Arc<[PathBuf]>>,
    pub(super) previews: BTreeMap<CallId, Value>,
}

impl Tools {
    /// 记下调用 `call_id` 判过的真实位置 `checked`。
    pub(crate) fn checked(&mut self, call_id: CallId, checked: Vec<PathBuf>) {
        self.checked.paths.insert(call_id, Arc::from(checked));
    }

    /// 记下调用 `call_id` 问人的卡片上那一份看一眼（施工 PK-3 上）：没看的不记。
    pub(crate) fn preview_for(&mut self, call_id: CallId, previewed: Option<Value>) {
        if let Some(previewed) = previewed {
            self.checked.previews.insert(call_id, previewed);
        }
    }

    /// 派的时候取走：没记下的（不该有）按一个都没判过算，工具换出来的一律不碰，不放开。
    pub(super) fn take_checked(&mut self, call_id: CallId) -> Arc<[PathBuf]> {
        self.checked
            .paths
            .remove(&call_id)
            .unwrap_or_else(|| Arc::from(Vec::new()))
    }

    /// 回合结束：判过却没派出去的（拒了的、人拒了的、打断了的）扔掉。调用都在回合里派，结束了的回合不会再派。
    pub(crate) fn forget_checked(&mut self) {
        self.checked.paths.clear();
        self.checked.previews.clear();
    }
}
