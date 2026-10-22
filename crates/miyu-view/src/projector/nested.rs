//! 脚本里调的工具（施工 M-5 上，`docs/blueprint/view.md`「`tool` 的 `nested`」）：开始了的瞬时事件 `tool.nested_started`、
//! 了结的 `tool.nested` 照编号认出父级：前台那次调用的那一步条目、后台任务的那一行，带上个数和最近一个；任务了结那条旁白
//! 带最后那一份。订阅时会话这一刻在跑的另照 [`Projector::nested_running`] 补上（头重连）。

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use miyu_kernel::event::{NestedStarted, ToolNested};
use miyu_kernel::id::{JobId, NestedId, Parent};
use miyu_kernel::session::NestedRunning;
use miyu_kernel::time::Timestamp;

use super::Projector;
use crate::change::Change;
use crate::entry::{Body, Nested, NestedCall, ToolState};
use crate::title;

/// 在跑的那一个的 `state`。
const RUNNING: &str = "running";

/// 每个父级下面见过的：键是父级的写法（调用编号或任务编号）。
#[derive(Debug, Default)]
pub(super) struct Seen {
    by_parent: BTreeMap<String, (BTreeSet<u32>, Nested)>,
}

impl Seen {
    /// 任务 `job` 下面到这一刻的那一份；没调过的没有。
    pub(super) fn of_job(&self, job: &JobId) -> Option<Nested> {
        self.by_parent
            .get(&job.to_string())
            .map(|(_, nested)| nested.clone())
    }
}

impl Projector {
    /// 订阅时会话这一刻在跑的嵌套调用（施工 M-5 上）：照开始了的补上，交回引起的变化。
    pub fn nested_running(&mut self, running: &[NestedRunning]) -> Vec<Change> {
        for call in running {
            self.nested_put(&call.id, &call.name, &call.args, RUNNING, Some(call.at));
        }
        self.drain()
    }

    /// 一个嵌套调用开始跑了（瞬时的，只在视图流里有）：参数这时还不知道，标题只有显示名。
    pub(super) fn nested_started(&mut self, at: Timestamp, started: &NestedStarted) {
        self.nested_put(&started.id, &started.name, "", RUNNING, Some(at));
    }

    /// 一个嵌套调用了结了。
    pub(super) fn nested_settled(&mut self, nested: &ToolNested) {
        self.nested_put(
            &nested.id,
            &nested.name,
            &nested.args,
            nested.status.as_str(),
            None,
        );
    }

    /// 记下这一个，序号是最大的就换成最近的那一个；照父级改那一步的条目或那一行任务。
    fn nested_put(
        &mut self,
        id: &NestedId,
        name: &str,
        args: &str,
        state: &str,
        started: Option<Timestamp>,
    ) {
        let parsed: Value = serde_json::from_str(args).unwrap_or(Value::Null);
        let shown = if state == RUNNING {
            ToolState::Running
        } else {
            ToolState::Ok
        };
        let call = NestedCall {
            n: id.n(),
            name: name.to_string(),
            title: title::of(name, &parsed, shown, None, &BTreeMap::new(), &self.texts),
            state: state.to_string(),
            started,
        };
        let key = match id.parent() {
            Parent::Call(call) => call.to_string(),
            Parent::Job(job) => job.to_string(),
        };
        let (seen, nested) = self
            .tracking
            .nested
            .by_parent
            .entry(key)
            .or_insert_with(|| {
                (
                    BTreeSet::new(),
                    Nested {
                        count: 0,
                        last: call.clone(),
                    },
                )
            });
        seen.insert(call.n);
        nested.count = u32::try_from(seen.len()).unwrap_or(u32::MAX);
        if call.n >= nested.last.n {
            nested.last = call;
        }
        let nested = nested.clone();
        match id.parent() {
            Parent::Call(call) => {
                if let Some(entry) = self.calls.get(call).cloned() {
                    self.touch(&entry, |entry| {
                        if let Body::Tool(tool) = &mut entry.body {
                            tool.nested = Some(nested);
                        }
                    });
                }
            }
            Parent::Job(job) => {
                if let Some(row) = self.row(job) {
                    row.nested = Some(nested);
                }
            }
        }
    }
}
