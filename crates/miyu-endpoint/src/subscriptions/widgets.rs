//! 界面块的订阅（施工 X-15 中，`docs/blueprint/widgets.md`「头」）：一个连接至多一个，再订阅换一个新的；回应经它写出去
//! （[`Target::Widgets`](super::Target::Widgets)），排在之后的推送前面。转发任务在 `widgets/head.rs`。

use std::sync::Arc;

use serde_json::Value;
use tokio::sync::mpsc;

use super::Subscriptions;
use crate::Core;
use crate::hello::Shaken;
use crate::widgets::head::{HeadForwarder, Looking};

impl Subscriptions {
    /// 订阅界面块：先起新的替掉原来的（新的先记成在看，旧的再不看，没人看的那一刻不会多出来），交回订阅的回应。
    pub(crate) fn add_widgets(
        &mut self,
        core: &Arc<Core>,
        looking: Looking,
        shaken: Shaken,
        out: &mpsc::Sender<String>,
    ) -> Value {
        let (forwarder, reply) =
            HeadForwarder::start(Arc::clone(core), looking, shaken, out.clone());
        self.widgets = Some(forwarder);
        reply
    }

    /// 取消订阅界面块：转发任务当场停，记下的不看了。
    pub(crate) fn remove_widgets(&mut self) {
        self.widgets = None;
    }
}
