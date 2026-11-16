//! 换了模型当场报新的（施工 MC-1，2026-11-16 项目主人定，`docs/blueprint/models.md`「怎么走」第六条第 3 条第 5 款）：人
//! `session.configure` 换了模型、思考强度，落了盘就照这一刻的配置预演下一个回合开始会怎么解析（`ModelPort::upcoming`），给头看的
//! `next` 换成它，变了推一条 `model.changed`（`why` 是 `turn`，同回合开始头看得到的变了）。头选完、切走再切回来（重新
//! `subscribe`）、`session.quota` 照的都是这一份。真发请求照旧下一个回合开始照新的：路由钉着的不动。
//!
//! 下一个回合开始真解析了，预演的清掉、照端口的；和刚报的一样的不再推，不一样的（池钉了成员、解析不出退回了）照旧推。预演着的
//! 时候这一轮里出错换了成员，不推 `failover`：头看的是下一轮的。

use miyu_kernel::event::{Body, ChangeWhy, Event};

use super::super::Actor;

impl Actor {
    /// 这一批里有带 `model` 或 `effort` 的 `session.policy_changed`（人换的、钉着的没了内核退回的）：照这一刻的配置预演，给头看
    /// 的接下来请求的模型变了推 `model.changed`。在写盘以前做（同换上下文上限，`store.rs`）：回了的命令、之后的 `subscribe`
    /// 读得到新的。
    pub(in crate::actor) async fn follow_configured(&mut self, events: &[Event]) {
        let configured = events.iter().any(|event| {
            matches!(&event.body, Body::PolicyChanged(changed)
                if changed.model.is_some() || changed.effort.is_some())
        });
        if !configured {
            return;
        }
        let config = self.config.peek(self.session.cwd().to_string()).await;
        let foreseen =
            self.model
                .upcoming(&config, self.session.reference(), self.session.effort());
        let before = self.shown_now().next;
        let after = {
            let mut shown = self
                .shown
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            shown.foreseen = foreseen;
            shown.refresh_next(&*self.model);
            shown.next.clone()
        };
        if after != before {
            self.announce(ChangeWhy::Turn);
        }
    }

    /// 回合开始、重新解析以前：预演的不要了，之后给头看的照端口真解析的。
    pub(in crate::actor) fn forget_foreseen(&self) {
        self.shown
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .foreseen = None;
    }

    /// 有没有预演着的。
    pub(in crate::actor) fn foreseeing(&self) -> bool {
        self.shown
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .foreseen
            .is_some()
    }
}
