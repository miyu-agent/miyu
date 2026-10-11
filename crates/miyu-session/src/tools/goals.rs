//! 长期目标（施工 X-8 上，`docs/blueprint/tools/goal.md`）：执行器这一头的目标端口。
//!
//! - 一次调用一个端口，只给本机的主会话（[`port`]，`Agents::lists_sessions`）：工具面上的 `goal` 也只有它们有。
//! - 工具交来要做的 → 经回传的通道送回 actor（[`Back::Goal`]）→ 看的当场答；改的交内核 `Goal` 命令、记下等它的回应，把等的口子
//!   交回来（[`GoalBack::Waiting`]），内核回应了这一头接着等到。
//! - 没答到就了结的（这次调用叫停了、掐掉了，会话停了）：口子丢掉，工具拿到 [`GoalRefused::Stopped`]。

use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use miyu_kernel::id::CallId;
use miyu_kernel::session::Outcome;
use miyu_tool::{GoalAnswering, GoalAsk, GoalPort, GoalRefused, GoalSeen};

use super::Tools;
use crate::port::Back;

/// actor 交回给端口的。
#[derive(Debug)]
pub(crate) enum GoalBack {
    /// 当场答了：看的，或者没交内核就拒了的。
    Answered(Result<GoalSeen, GoalRefused>),
    /// 交了内核：改之前的样子，等它回应的口子。
    Waiting {
        seen: GoalSeen,
        outcome: oneshot::Receiver<Outcome>,
    },
}

/// 一次调用的目标端口：交来的照调用编号送回 actor。
struct Goaler {
    call_id: CallId,
    backs: mpsc::UnboundedSender<Back>,
}

impl GoalPort for Goaler {
    fn goal(&self, ask: GoalAsk) -> GoalAnswering<'_> {
        let (reply, answered) = oneshot::channel();
        send(
            &self.backs,
            Back::Goal {
                call_id: self.call_id,
                ask,
                reply,
            },
        );
        Box::pin(async move {
            match answered.await {
                Ok(GoalBack::Answered(answer)) => answer,
                Ok(GoalBack::Waiting { seen, outcome }) => match outcome.await {
                    Ok(Outcome::Accepted { .. }) => Ok(seen),
                    Ok(Outcome::Rejected { reason, .. }) => Err(GoalRefused::Kernel(reason)),
                    Ok(_) | Err(_) => Err(GoalRefused::Stopped),
                },
                Err(_) => Err(GoalRefused::Stopped),
            }
        })
    }
}

/// 交给第 `call_id` 次调用的目标端口：本机的主会话（`goals`，造会话、载入时照 `Agents::lists_sessions` 定的）才有。
pub(super) fn port(
    goals: bool,
    call_id: CallId,
    backs: &mpsc::UnboundedSender<Back>,
) -> Option<Arc<dyn GoalPort>> {
    goals.then(|| {
        Arc::new(Goaler {
            call_id,
            backs: backs.clone(),
        }) as Arc<dyn GoalPort>
    })
}

impl Tools {
    /// 第 `call_id` 次调用还在跑、没叫它停过：它交来的目标改动才办。
    pub(crate) fn live(&self, call_id: CallId) -> bool {
        self.running
            .get(&call_id)
            .is_some_and(|running| !running.stopping)
    }
}

/// 送回 actor。actor 停了就没人收：工具那一头等不到，照没答收场。
#[expect(
    clippy::let_underscore_must_use,
    reason = "actor 停了：送不到就是没人要，工具那一头照没答收场"
)]
fn send(backs: &mpsc::UnboundedSender<Back>, back: Back) {
    let _ = backs.send(back);
}
