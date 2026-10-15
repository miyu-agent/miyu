//! 回合开始问扩展要加的字（施工 X-16，`docs/blueprint/extensions.md`「回合开始的挂接点」）：记忆、知识库的挂接点之后问协议
//! 端点交进来的端口，交回的每一块换成内核的注入，`by` 是那个包。

use miyu_kernel::event::ContextInjected;
use miyu_kernel::id::{FactKind, ModuleId};
use miyu_kernel::session::Injection;
use std::future::Future;
use std::sync::Arc;

use miyu_tool::{HooksPort, Inject, TurnAsk, Where};

use super::Actor;

impl Actor {
    /// 问扩展：没交端口的、测试里造的（没有人格的几层）不问。`text` 是触发这一轮的人话。问之前把要的都拿出来，等的时候不
    /// 借着 actor（它不是 `Sync`）。
    pub(super) fn extension_injections(
        &self,
        text: Option<String>,
    ) -> impl Future<Output = Vec<Injection>> + Send + 'static {
        let asking = self.turn_ask(text);
        async move {
            let Some((port, ask)) = asking else {
                return Vec::new();
            };
            port.turn_start(ask)
                .await
                .into_iter()
                .filter_map(injection)
                .collect()
        }
    }

    /// 这一轮要问的端口和问的话。
    fn turn_ask(&self, text: Option<String>) -> Option<(Arc<dyn HooksPort>, TurnAsk)> {
        let ((port, session), refresh) = (self.tools.hooks()?, self.persona.as_ref()?);
        let venue = match (refresh.venue.as_str(), refresh.snapshot.group.is_some()) {
            ("local", _) => Where::Local,
            (_, true) => Where::Group,
            (_, false) => Where::Private,
        };
        let ask = TurnAsk {
            session: session.as_str().to_string(),
            venue,
            text,
            tools: refresh
                .snapshot
                .tools
                .iter()
                .map(|tool| tool.name.clone())
                .collect(),
        };
        Some((port, ask))
    }
}

/// 端口交的一块换成内核的注入；包的编号、类别不合写法的不要（端口查过，这里再守一道）。
fn injection(inject: Inject) -> Option<Injection> {
    Some(Injection {
        module: ModuleId::parse(&inject.package).ok()?,
        fact: ContextInjected {
            kind: FactKind::parse(&inject.kind).ok()?,
            text: inject.text,
            refs: Vec::new(),
            after: true,
        },
    })
}
