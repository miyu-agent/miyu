//! 会话中途换人格、预设（recast）（施工 P-13，`docs/designs/16-人格与预设.md` Y5，2026-10-17 项目主人改）：人换，内核记下要换成的，马上
//! 单开一轮压缩（同手动压缩：摘要照旧人格的前缀写）；真换在下一个回合开始时，由会话那一层照 [`Session::persona`]、
//! [`Session::preset`] 重拼快照。压缩那一轮不跑回合开始的挂接点，压缩的请求一定还是旧人格。
//!
//! 有回合在进行的拒绝（`turn_running`）；和现在一样的格去掉，一格都不剩的接受、什么都不记；没有能压的不开那一轮。

use super::action::{Action, Reason};
use super::{Session, accepted, rejected};
use crate::event::{Body, PolicyChanged};
use crate::id::CommandId;
use crate::origin::By;
use crate::time::Timestamp;

impl Session {
    /// 会话现在要用的人格（施工 P-13）：`session.created` 的，被后来换的盖掉；无人格的没有。只读：会话那一层回合开始照它看要
    /// 不要换快照。
    pub fn persona(&self) -> Option<&str> {
        self.reference.persona()
    }

    /// 会话现在要用的预设（施工 P-13）：同上；以前的日志没记的没有。
    pub fn preset(&self) -> Option<&str> {
        self.reference.preset()
    }

    /// 换人格、预设：`persona` 外层没有是不换，`Some(None)` 是换成无人格；`preset` 没有是不换。见模块的说明。
    pub(super) fn recast(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        (persona, preset): (Option<Option<String>>, Option<String>),
    ) -> Vec<Action> {
        if self.turn.is_some() {
            return vec![rejected(id, Reason::TurnRunning)];
        }
        let persona = persona
            .filter(|persona| persona.as_deref() != self.persona())
            .map(Option::unwrap_or_default);
        let preset = preset.filter(|preset| Some(preset.as_str()) != self.preset());
        if persona.is_none() && preset.is_none() {
            self.recent.insert(id.clone(), Vec::new());
            return vec![accepted(id, Vec::new())];
        }
        let body = Body::PolicyChanged(PolicyChanged {
            persona,
            preset,
            ..PolicyChanged::default()
        });
        let mut events = vec![self.record(at, by, Some(id.clone()), body)];
        if let Some(upto) = self.manual_upto() {
            events.push(self.open_manual(&id, at, upto, None));
        }
        self.accept(id, events.iter().map(|event| event.seq).collect());
        vec![Action::Append(events)]
    }
}
