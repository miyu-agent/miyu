//! 场所一段语音的上限（施工 VM-2，`docs/blueprint/venues.md`「`venue.session`」）：造场所会话时记在 `session.created` 的
//! `voice_max_ms`；桥找回会话时报的和现在的不一样（含造的时候没报的），记一条只写 `voice_max_ms` 的 `session.policy_changed`。
//!
//! 照日志算（`configure.rs` 的 `Reference`）：后来的盖掉前面的，撤掉的回合里的也算（这是场所的事实，不是对话的一部分）。
//! 只设不去：桥没报的照记过的，所以没有「去掉」的写法。会话那一层派调用时照 [`Session::voice_max_ms`] 交给扩展。

use super::action::Action;
use super::{Session, accepted};
use crate::event::{Body, PolicyChanged};
use crate::id::CommandId;
use crate::origin::By;
use crate::time::Timestamp;

impl Session {
    /// 场所一段语音至多多长，毫秒（施工 VM-2）：`session.created` 的，被后来带 `voice_max_ms` 的 `session.policy_changed` 盖掉；
    /// 没有上限的没有。只读：会话那一层派调用时照它交给扩展（`miyu_tool::Caller::voice_max_ms`）。
    pub fn voice_max_ms(&self) -> Option<u64> {
        self.reference.voice()
    }

    /// 换场所一段语音的上限（施工 VM-2）：和现在的一样的接受，什么都不记，编号照样记下；不一样的记一条
    /// `session.policy_changed`，只写 `voice_max_ms`，`by` 是报的那个桥，回合进行中的带上这个回合。正不正协议那一头查过。
    pub(super) fn set_voice_limit(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        ms: u64,
    ) -> Vec<Action> {
        if self.voice_max_ms() == Some(ms) {
            self.recent.insert(id.clone(), Vec::new());
            return vec![accepted(id, Vec::new())];
        }
        let body = Body::PolicyChanged(PolicyChanged {
            voice_max_ms: Some(ms),
            ..PolicyChanged::default()
        });
        let event = self.record(at, by, Some(id.clone()), body);
        self.accept(id, vec![event.seq]);
        vec![Action::Append(vec![event])]
    }
}
