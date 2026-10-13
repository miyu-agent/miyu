//! 看守查撤掉重发（施工 4-15，`session/supersede.rs`）：撤掉的只有一个字都还没回的主请求，用量、金额都没有，前面紧挨着的
//! 是撤掉它的那条回合里来的消息；后面那一次主请求是重发，不算步数。

use super::*;
use crate::event::ModelCalled;

impl Watch {
    /// 一条结果是撤掉重发的 `model.called`；`before` 是这一批里它前面那一条。
    pub(super) fn superseded_called(&mut self, called: &ModelCalled, before: Option<&Event>) {
        let seed = self.seed;
        self.seen_paths.insert("撤掉重发");
        assert!(
            !self.retries.spoke.contains(&called.seen),
            "种子 {seed}：回过字的请求 {} 也撤掉了",
            called.seen
        );
        assert_eq!(called.usage, None, "种子 {seed}：撤掉重发的请求不该有用量");
        assert!(
            before.is_some_and(|event| matches!(event.body, Body::MessageUser(_))
                && event.turn.is_some()
                && event.seq > called.seen),
            "种子 {seed}：撤掉重发的紧跟在回合里来的那条消息后面：{before:?}"
        );
        self.retries.superseded = true;
    }

    /// 在路上的那次发出去了、一个字都还没回：随机输入这时多半先来增量（`random.rs`），不然常常撤掉重发，走不到调工具的
    /// 那些路。
    pub(in super::super) fn silent(&self) -> bool {
        self.asking
            .is_some_and(|seen| self.sent.contains(&seen) && !self.retries.spoke.contains(&seen))
    }

    /// 发主请求：撤掉重发以后的那一次是重发，不算步数。交回是不是。
    pub(super) fn superseded_resend(&mut self) -> bool {
        std::mem::take(&mut self.retries.superseded)
    }
}
