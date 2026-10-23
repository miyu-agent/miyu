//! 替身怎么回嵌套调用（施工 M-1，`docs/blueprint/kernel/session.md`「嵌套调用」）：脚本那头由测试扮，`nested_call` 送进一次
//! `miyu call`；链照排好的结论回（没排的放行）；派去跑的停住，等测试 `nested_done` 交回；掐掉的、答脚本的都记下来。

use std::collections::VecDeque;

use super::Stage;
use crate::id::{ContentHash, NestedId};
use crate::raw::RawJson;
use crate::session::{Input, NestedAction, NestedAnswer, NestedInput, Verdict};

/// 替身手里的嵌套调用。
#[derive(Debug, Default)]
pub(super) struct Bench {
    /// 链的结论，照先后取；没排的放行。
    verdicts: VecDeque<Verdict>,
    /// 交给链的，照先后。
    guarded: Vec<NestedId>,
    /// 交给链时带的本会话放行过的规则，照先后（施工 M-7 上）。
    grants: Vec<(NestedId, Vec<RawJson>)>,
    /// 扣着链的结论：交给链的不当场答，等 [`Stage::nested_verdict`]（施工 M-7 上）。
    holding_guards: bool,
    /// 派去跑、还停着的。
    running: Vec<NestedId>,
    /// 派去跑过的：编号、工具名、参数。
    ran: Vec<(NestedId, String, String)>,
    /// 叫掐掉的。
    cancelled: Vec<NestedId>,
    /// 扣着落盘：追加了不送回「落盘了」，等 [`Stage::release_stores`]。
    holding: bool,
    /// 答脚本那头的，照先后。
    answers: Vec<(NestedId, NestedAnswer)>,
}

impl Stage {
    /// 排好链对嵌套调用的结论，照先后用；用完了的放行。
    pub fn nested_guards(&mut self, verdicts: impl IntoIterator<Item = Verdict>) {
        self.nested.verdicts.extend(verdicts);
    }

    /// 脚本里来了一次 `miyu call`：编号写成 `call_4_1#1`、`j1#2`。
    ///
    /// # Panics
    ///
    /// 编号的写法不对。
    pub fn nested_call(&mut self, id: &str, name: &str, args: &str) {
        let id = NestedId::parse(id).unwrap_or_else(|e| panic!("嵌套调用编号的写法坏了：{e}"));
        let at = self.tick();
        self.run(Input::Nested(NestedInput::Call {
            at,
            id,
            name: name.to_string(),
            args: args.to_string(),
        }));
    }

    /// 停着的嵌套调用跑完了：`error` 的是出错；结果是一份 7 个字节的 blob。
    ///
    /// # Panics
    ///
    /// 它没在跑。
    pub fn nested_done(&mut self, id: &str, error: bool) {
        let id = NestedId::parse(id).unwrap_or_else(|e| panic!("嵌套调用编号的写法坏了：{e}"));
        let k = self
            .nested
            .running
            .iter()
            .position(|running| *running == id)
            .unwrap_or_else(|| panic!("{id} 没在跑"));
        self.nested.running.remove(k);
        let at = self.tick();
        self.run(Input::Nested(NestedInput::Done {
            at,
            id,
            error,
            duration_ms: Some(12),
            result: Some(ContentHash::of(b"result\n")),
            bytes: Some(7),
        }));
    }

    /// 已经交回的嵌套调用又交一次：迟到的、不在跑的，内核该不理。
    ///
    /// # Panics
    ///
    /// 编号的写法不对。
    pub fn nested_done_late(&mut self, id: &str) {
        let id = NestedId::parse(id).unwrap_or_else(|e| panic!("嵌套调用编号的写法坏了：{e}"));
        let at = self.tick();
        self.run(Input::Nested(NestedInput::Done {
            at,
            id,
            error: false,
            duration_ms: None,
            result: None,
            bytes: None,
        }));
    }

    /// 扣着落盘：之后追加的先不送回「落盘了」（查「落了盘才答」）。
    pub fn hold_stores(&mut self) {
        self.nested.holding = true;
    }

    /// 不再扣着：追加过的都落盘了。
    pub fn release_stores(&mut self) {
        self.nested.holding = false;
        let upto = self.log.last().map(|event| event.seq);
        if let Some(upto) = upto {
            let at = self.tick();
            self.run(Input::Stored { at, upto });
        }
    }

    /// 扣着落盘，只把到第 `upto` 条的落了盘（查「落了盘才答」：只落一部分）。
    ///
    /// # Panics
    ///
    /// `upto` 是 0。
    pub fn store_upto(&mut self, upto: u64) {
        let upto = crate::id::Seq::new(upto).unwrap_or_else(|| panic!("序号从 1 起"));
        let at = self.tick();
        self.run(Input::Stored { at, upto });
    }

    /// 扣着落盘没有。
    pub(super) fn holding_stores(&self) -> bool {
        self.nested.holding
    }

    /// 内核这一刻说在跑的嵌套调用（订阅时交给头的那一份）。
    pub fn nested_in_flight(&self) -> Vec<String> {
        self.session
            .nested_running()
            .iter()
            .map(|started| format!("{} {}", started.id, started.name))
            .collect()
    }

    /// 交给链的嵌套调用，照先后。
    pub fn nested_guarded(&self) -> &[NestedId] {
        &self.nested.guarded
    }

    /// 派去跑、还停着的嵌套调用。
    pub fn nested_running(&self) -> &[NestedId] {
        &self.nested.running
    }

    /// 派去跑过的嵌套调用：编号、工具名、参数。
    pub fn nested_ran(&self) -> &[(NestedId, String, String)] {
        &self.nested.ran
    }

    /// 叫掐掉的嵌套调用。
    pub fn nested_cancelled(&self) -> &[NestedId] {
        &self.nested.cancelled
    }

    /// 答脚本那头的，照先后。
    pub fn nested_answers(&self) -> &[(NestedId, NestedAnswer)] {
        &self.nested.answers
    }

    /// 扣着链的结论（施工 M-7 上）：之后交给链的不当场答，由 [`Self::nested_verdict`] 一件一件交，几件能同时在过链。
    pub fn hold_nested_guards(&mut self) {
        self.nested.holding_guards = true;
    }

    /// 交回链对 `id` 的结论（扣着的时候用）。
    ///
    /// # Panics
    ///
    /// 编号的写法不对。
    pub fn nested_verdict(&mut self, id: &str, verdict: Verdict) {
        let id = NestedId::parse(id).unwrap_or_else(|e| panic!("嵌套调用编号的写法坏了：{e}"));
        let at = self.tick();
        self.run(Input::Nested(NestedInput::Guarded { at, id, verdict }));
    }

    /// 交给链时带的本会话放行过的规则：编号、规则，照先后（施工 M-7 上）。
    #[must_use]
    pub fn nested_grants(&self) -> &[(NestedId, Vec<RawJson>)] {
        &self.nested.grants
    }

    /// 回一个嵌套调用的动作。
    pub(super) fn act_nested(&mut self, action: NestedAction) -> Vec<Input> {
        match action {
            NestedAction::Guard { id, grants, .. } => {
                self.nested.guarded.push(id.clone());
                self.nested.grants.push((id.clone(), grants));
                if self.nested.holding_guards {
                    return Vec::new();
                }
                let verdict = self.nested.verdicts.pop_front().unwrap_or(Verdict::Allow);
                vec![Input::Nested(NestedInput::Guarded {
                    at: self.tick(),
                    id,
                    verdict,
                })]
            }
            NestedAction::Run { id, name, args, .. } => {
                self.nested.ran.push((id.clone(), name, args));
                self.nested.running.push(id);
                Vec::new()
            }
            NestedAction::Cancel { id } => {
                self.nested.running.retain(|running| *running != id);
                self.nested.cancelled.push(id);
                Vec::new()
            }
            NestedAction::Answer { id, answer } => {
                self.nested.answers.push((id, answer));
                Vec::new()
            }
        }
    }

    /// 执行器没了（崩了、重启）：在跑的跟着没了。
    pub(super) fn drop_nested(&mut self) {
        self.nested.running.clear();
    }
}
