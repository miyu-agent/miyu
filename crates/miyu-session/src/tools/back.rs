//! 跑工具的任务送回来的，写成内核的输入（从 `tools.rs` 挪来，施工 D-2：那边要放提问的端口，过了 500 行）。

use std::path::PathBuf;
use std::sync::Arc;

use miyu_kernel::event::Effect;
use miyu_kernel::raw::RawJson;
use miyu_kernel::session::Input;
use miyu_kernel::time::Timestamp;

use super::{ToolBack, Tools, text};
use crate::TARGET;
use crate::effects;
use crate::lines::millis;

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
                    blocks: done.blocks,
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
