//! 读推送等到某一刻（施工 V-39 下从 `mod.rs` 挪来：那一份到了行数上限）。

use std::sync::Arc;

use miyu_kernel::event::{Body, TransientBody};
use miyu_session::{Pushed, Subscription};

use super::within;

/// 一直读推送，读到第一段增量：请求在读流了。
pub async fn until_delta(subscription: &mut Subscription) {
    within("增量", async {
        loop {
            let next = subscription.next().await.expect("订阅没断");
            if matches!(&*next, Pushed::Transient(t) if matches!(t.body, TransientBody::ModelDelta(_)))
            {
                return;
            }
        }
    })
    .await;
}

/// 一直读推送，读到回合结束那一条为止，交回读到的每一份。
pub async fn until_turn_ends(subscription: &mut Subscription) -> Vec<Arc<Pushed>> {
    let mut pushed = Vec::new();
    loop {
        let next = within("推送", subscription.next()).await.expect("订阅没断");
        let ended = matches!(
            &*next,
            Pushed::Events(events) if events.iter().any(|event| matches!(event.body, Body::TurnEnded(_)))
        );
        pushed.push(next);
        if ended {
            return pushed;
        }
    }
}
