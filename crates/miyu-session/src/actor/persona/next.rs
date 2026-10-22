//! 下一次请求会发的工具（施工 S-T，`session.tools`）：照回合开始换快照那一套（[`super::look`]）看一遍，人格、预设、目录、
//! 知识库换了的照换了以后的算，没换的照现在的快照。只看不换：换不换照旧等回合开始。看的这一遍在阻塞线程里，不挡收件箱。

use tokio::sync::oneshot;

use miyu_policy::{Offered, Snapshot};

use super::recast::Wanted;
use super::{Actor, Seen, look};
use crate::blocking::blocking;

impl Actor {
    /// 交回下一次请求会发的工具。没有人格快照的（测试里造的）是空的；裸的（整理会话）照现在的快照。
    pub(in crate::actor) fn next_tools(&self, reply: oneshot::Sender<Vec<Offered>>) {
        let Some(refresh) = self.persona.clone() else {
            super::super::answer(reply, Vec::new());
            return;
        };
        if refresh.snapshot.bare {
            super::super::answer(reply, offered(&refresh.snapshot));
            return;
        }
        let values = self.config.current().resolved.values();
        let now = refresh.tools.edition();
        let asked = self
            .tools
            .hooks()
            .map(|(port, session)| port.appended(session.as_str()))
            .unwrap_or_default();
        let project = self.session.project().map(str::to_string);
        let wanted = Wanted::of(&self.session, &refresh.snapshot, refresh.child);
        tokio::spawn(async move {
            let tools =
                blocking(
                    move || match look(&refresh, &values, &now, (asked, project, wanted)) {
                        Seen::Swapped(snapshot, ..) => offered(&snapshot),
                        _ => offered(&refresh.snapshot),
                    },
                )
                .await;
            super::super::answer(reply, tools);
        });
    }
}

fn offered(snapshot: &Snapshot) -> Vec<Offered> {
    miyu_policy::offered(&snapshot.tools, snapshot.core.loading.as_ref())
}
