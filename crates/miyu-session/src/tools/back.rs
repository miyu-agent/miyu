//! 跑工具的任务送回来的，写成内核的输入（从 `tools.rs` 挪来，施工 D-2：那边要放提问的端口，过了 500 行）。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tokio::task::AbortHandle;

use miyu_kernel::event::{Effect, Question, Response};
use miyu_kernel::id::CallId;
use miyu_kernel::raw::RawJson;
use miyu_kernel::session::Input;
use miyu_kernel::time::Timestamp;
use miyu_tool::{Done, Stop};

use super::{Tools, text};
use crate::TARGET;
use crate::effects;
use crate::lines::millis;
use crate::unsandboxed::Grant;

/// 一次调用怎么套沙盒（施工 X-37 补）。
pub(crate) enum Wrap {
    /// 套。
    Sandbox,
    /// 不套：工具报要在沙盒外跑的（权限策略照同一个报判过了），管理员放行了的、不是一个文件的工具。
    Bare,
    /// 管理员放行了的、带文件的工具：文件（真实的位置）落在这一次沙盒的规格写得了的地方（`write`）的照旧套，不在的不套。权限
    /// 策略照它的边界表判过同一条（`guard/unsandboxed.rs`）。
    BareUnlessWritable(PathBuf),
}

/// 一次在跑的调用。
pub(super) struct Running {
    pub(super) task: AbortHandle,
    /// 交给工具的那面旗。
    pub(super) stop: Stop,
    /// 叫它停过：还没交回来就被掐掉的，改动可能不留效果，记一行 `WARN`。
    pub(super) stopping: bool,
    pub(super) started: Instant,
    pub(super) name: String,
    /// 派它那一刻这件工具给人看的名字（施工 9-8 六补）：送回来时随结果记进日志。
    pub(super) display_name: BTreeMap<String, String>,
    /// 它交了题、在等回答（施工 D-2）：回答落了盘从这里送回去。叫停、掐掉、了结时跟着丢，工具当没答收场。
    pub(super) answer: Option<tokio::sync::oneshot::Sender<Vec<Response>>>,
}

/// 跑工具的任务送回来的。
#[derive(Debug)]
pub(crate) enum ToolBack {
    /// 交了一组题，回答落了盘从 `reply` 送回去（施工 D-2，`questions.rs`）。
    Asks {
        call_id: CallId,
        questions: Vec<Question>,
        reply: tokio::sync::oneshot::Sender<Vec<Response>>,
    },
    /// 执行中的一段输出。
    Progress { call_id: CallId, text: String },
    /// 跑完了：工具交回的，和它报的效果（改前改后已经存成了 blob）。
    Done {
        call_id: CallId,
        done: Box<Done>,
        effects: Vec<Effect>,
        /// 碰了的文件上面子目录的项目指令，找到了的（施工 P-14 下）。
        docs: Option<super::docs::Below>,
    },
    /// 工具自己崩了（panic）。
    Crashed { call_id: CallId },
}

impl Wrap {
    /// 要先写出沙盒的规格（套的、要看规格才定的）。
    pub(crate) fn needs_spec(&self) -> bool {
        !matches!(self, Wrap::Bare)
    }

    /// 看规格才定的那个文件。
    pub(crate) fn unless(self) -> Option<PathBuf> {
        match self {
            Wrap::BareUnlessWritable(real) => Some(real),
            Wrap::Sandbox | Wrap::Bare => None,
        }
    }
}

impl Tools {
    /// 结果没人要了（调用已经被掐掉）里派出去的子代理（施工 7-5 补）：没记成任务，没人管，停掉它。
    fn unclaimed(&self, effects: &[Effect]) {
        let Some(agents) = self.agents() else {
            return;
        };
        for (job, child) in crate::agents::spawned_in(effects) {
            agents.stop_unclaimed(&job, child);
        }
    }

    /// 换上这一刻开着的技能文件夹和它们给人看的名字（施工 X-11 下、9-8 五补）：认用过的技能、沙盒都照新的。
    pub(crate) fn set_shelves(&mut self, shelves: Vec<PathBuf>, names: crate::skills::Names) {
        if let Some(sandbox) = self.sandbox.as_mut() {
            sandbox.set_shelves(shelves.clone());
        }
        self.shelves = shelves;
        self.names = names;
    }

    /// 换上这一轮的私密目录（施工 5-15 下）：沙盒照它藏。
    pub(crate) fn set_private(&mut self, private: Vec<PathBuf>) {
        if let Some(sandbox) = self.sandbox.as_mut() {
            sandbox.set_private(private);
        }
    }

    /// 换上这一轮的沙盒强度（施工 5-16 下）。
    pub(crate) fn set_weak(&mut self, weak: bool) {
        if let Some(sandbox) = self.sandbox.as_mut() {
            sandbox.set_weak(weak);
        }
    }

    /// 换上这一轮管理员放行出沙盒的（施工 X-36）：之后派的调用照新的写不写沙盒的规格。
    pub(crate) fn set_unsandboxed(&mut self, unsandboxed: crate::unsandboxed::Unsandboxed) {
        self.unsandboxed = unsandboxed;
    }

    /// 这一次怎么套沙盒：工具报要在沙盒外跑的（施工 D-4）、管理员放行了的（施工 X-36）不套；放行了的带文件的，文件落在这一次
    /// 沙盒写得了的地方的照旧套（施工 X-37 补，[`Wrap::BareUnlessWritable`]）。
    pub(crate) fn wrap(
        &self,
        name: &str,
        tool: &Arc<dyn miyu_tool::Tool>,
        call: &miyu_tool::Call,
    ) -> Wrap {
        if tool.outside_sandbox(call) {
            return Wrap::Bare;
        }
        match self.unsandboxed.grant(name, &self.catalog) {
            Grant::No => Wrap::Sandbox,
            Grant::Whole => Wrap::Bare,
            Grant::File(real) => Wrap::BareUnlessWritable(real),
        }
    }

    /// 跑工具的任务送回来的，写成内核的输入。不在跑的（已经叫停了的）不理，派出去的子代理停掉（施工 7-5 补）。
    pub(crate) fn back(&mut self, at: Timestamp, back: ToolBack) -> Option<Input> {
        match back {
            ToolBack::Asks {
                call_id,
                questions,
                reply,
            } => self.asked(at, call_id, questions, reply),
            ToolBack::Progress { call_id, text } => self
                .running
                .contains_key(&call_id)
                .then_some(Input::ToolProgress { at, call_id, text }),
            ToolBack::Done {
                call_id,
                done,
                effects,
                docs,
            } => {
                let Some(running) = self.running.remove(&call_id) else {
                    self.unclaimed(&effects);
                    return None;
                };
                let effects = effects::skilled(effects, &self.shelves, &self.names);
                effects::saw(Arc::make_mut(&mut self.seen), &effects);
                let took_ms = millis(running.started.elapsed());
                tracing::info!(
                    target: TARGET,
                    call = call_id.to_string().as_str(),
                    took_ms,
                    error = done.error.then_some(true),
                    stopped = done.stopped.then_some(true),
                    "ran"
                );
                Some(Input::ToolDone {
                    at,
                    call_id,
                    error: done.error,
                    // 没交过的子目录项目指令接在结果后面（施工 P-14 下）。
                    blocks: done
                        .blocks
                        .into_iter()
                        .chain(self.docs.tell(docs))
                        .collect(),
                    duration_ms: Some(took_ms),
                    human: done.human,
                    effects,
                    stopped: done.stopped,
                    end_turn: done.end_turn,
                    sent: done.sent,
                    display_name: running.display_name,
                    // 给头看的结构化结果（施工 W-11）：工具交的照原样记。
                    detail: done.detail.as_ref().and_then(RawJson::of),
                })
            }
            ToolBack::Crashed { call_id } => {
                let running = self.running.remove(&call_id)?;
                let took_ms = millis(running.started.elapsed());
                tracing::error!(
                    target: TARGET,
                    call = call_id.to_string().as_str(),
                    tool = running.name.as_str(),
                    took_ms,
                    "crashed"
                );
                let worded = self.lettering.run().crashed(&running.name);
                Some(Input::ToolDone {
                    at,
                    call_id,
                    error: true,
                    blocks: text(worded.text),
                    duration_ms: Some(took_ms),
                    human: worded.said,
                    effects: Vec::new(),
                    stopped: false,
                    end_turn: false,
                    sent: Vec::new(),
                    display_name: running.display_name,
                    detail: None,
                })
            }
        }
    }
}
