//! 嵌套调用的几条（施工 M-1，设计 `39-脚本调工具.md` 第四节、第七节，蓝图 `kernel/history.md`「账本查的规矩」）。
//!
//! 嵌套调用没有「开始」的事件，账本只看得到了结的 `tool.nested`：「父级结束之前挂在下面的都已了结」由内核在记父级的结果、
//! 报告之前收齐（`session/nested.rs`），这里守的是父级还活着才收、同一个编号不收两次。记的只有还活着的父级下面了结过的
//! 序号，父级一结束就清掉：只随在跑的调用、任务数长，不随日志长。

use std::collections::{BTreeMap, BTreeSet};

use crate::event::{Body, Event, ToolNested};
use crate::id::Parent;

/// 还活着的父级下面了结过的序号。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Settled(BTreeMap<Parent, BTreeSet<u32>>);

impl Settled {
    /// 这个编号在它的父级下面没记过。
    pub(super) fn check(&self, nested: &ToolNested) -> Result<(), String> {
        let seen = self
            .0
            .get(nested.id.parent())
            .is_some_and(|settled| settled.contains(&nested.id.n()));
        match seen {
            true => Err(format!("nested call {} is already settled", nested.id)),
            false => Ok(()),
        }
    }

    /// 记下了结过的；父级的结果、报告到了，它下面的清掉。
    pub(super) fn record(&mut self, event: &Event) {
        match &event.body {
            Body::ToolNested(nested) => {
                self.0
                    .entry(nested.id.parent().clone())
                    .or_default()
                    .insert(nested.id.n());
            }
            Body::ToolResult(result) => {
                self.0.remove(&Parent::Call(result.call_id));
            }
            Body::JobReported(reported) => {
                self.0.remove(&Parent::Job(reported.job.clone()));
            }
            _ => {}
        }
    }
}
