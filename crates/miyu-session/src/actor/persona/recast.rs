//! 中途换人格、预设（施工 P-13，`docs/designs/16-人格与预设.md` Y5）：内核记下要换成的（`Session::persona`、
//! `Session::preset`），压缩那一轮照旧；下一个回合开始照它们重拼快照：人格的字换成新人格的，预设照新的重新筛工具面（记忆
//! 开不开照开会话时的）。跟着人格的记忆挪到新人格那一间，从换的那一条起算（`memory/recast.rs`）；知识库照新人格查。
//! 新人格、新预设找不着的照旧，记一行，下一个回合再试。

use std::sync::Arc;

use miyu_config::Values;
use miyu_kernel::session::Session;
use miyu_policy::memory::MemoryScope;
use miyu_policy::preset::Chosen;
use miyu_policy::{PersonaTexts, Snapshot};
use miyu_store::personas::PersonaError;
use miyu_store::recall::NO_PERSONA;

use super::super::Actor;
use super::{Refresh, Seen};
use crate::TARGET;
use crate::blocking::blocking;
use crate::knowledge::Shelves;
use crate::store::Rewire;

/// 要换成什么（施工 P-13）：内核记下的和快照里的不一样的那几格。
#[derive(Default)]
pub(super) struct Wanted {
    /// 换人格：里面没有的是换成无人格。
    pub(super) persona: Option<Option<String>>,
    /// 换预设。
    pub(super) preset: Option<String>,
}

impl Wanted {
    /// 照内核 `session` 记下的和快照 `snapshot` 比；子会话不换（协议那一头也不让换）。
    pub(super) fn of(session: &Session, snapshot: &Snapshot, child: bool) -> Wanted {
        if child {
            return Wanted::default();
        }
        let persona = (session.persona() != snapshot.persona.as_deref())
            .then(|| session.persona().map(str::to_string));
        // 快照现在的预设：无工具的照内核的写法是空的编号（施工 P-16 下）。
        let now = match snapshot.no_tools {
            true => Some(""),
            false => snapshot.preset.as_ref().map(|pin| pin.id.as_str()),
        };
        let preset = session
            .preset()
            .filter(|id| now.is_none_or(|now| now != *id))
            .map(str::to_string);
        Wanted { persona, preset }
    }
}

/// 新人格 `persona` 的字：无人格的是空的。找不着的照旧不换。
pub(super) fn texts(refresh: &Refresh, persona: Option<&str>) -> Result<PersonaTexts, Seen> {
    let Some(id) = persona else {
        return Ok(PersonaTexts::default());
    };
    match refresh.personas.find(id) {
        Ok(found) => Ok(found.texts),
        Err(PersonaError::NotFound(_)) => Err(Seen::Kept(format!("persona {id} not found"))),
        Err(error) => Err(Seen::Unreadable(error.to_string())),
    }
}

/// 新预设 `id`：照现在的几层找；记忆开不开照开会话时的（快照里的范围），场所显示不了图片的照旧扣下要图的包；全局开关照这一轮
/// 的配置 `values`（施工 X-27）。
pub(super) fn preset(refresh: &Refresh, id: &str, values: &Values) -> Result<Chosen, String> {
    let places = refresh
        .presets
        .as_ref()
        .ok_or_else(|| "presets not available".to_string())?;
    let found = places.presets.find(id).map_err(|error| error.to_string())?;
    let features = &places.features;
    let memory = refresh.snapshot.memory_scope() != MemoryScope::Off;
    let file = found.file.following(crate::switches::of(values));
    let chosen = Chosen::new(found.id, file, features).keeping_memory(memory, features);
    Ok(match refresh.images {
        true => chosen,
        false => chosen.blind(features),
    })
}

/// 换了人格的知识库：照新人格查。没接知识库的没有。
pub(super) fn shelves(refresh: &Refresh, persona: Option<&str>) -> Option<Arc<Shelves>> {
    let shelves = refresh.knowledge.as_ref()?;
    Some(Arc::new(shelves.recast(persona.map(str::to_string))))
}

impl Actor {
    /// 快照换成了人格 `persona` 的：知识库换成新人格的；跟着人格的记忆挪到它那一间，回合索引照日志重接。读不了日志的这个会话
    /// 不再记，记一行。
    pub(super) async fn rehome(&mut self, persona: Option<String>, shelves: Option<Arc<Shelves>>) {
        let Some(refresh) = self.persona.as_mut() else {
            return;
        };
        if shelves.is_some() {
            refresh.knowledge.clone_from(&shelves);
        }
        let (personas, venue, cell) = (
            refresh.personas.clone(),
            refresh.venue.clone(),
            Arc::clone(&refresh.room),
        );
        let moved = self
            .tools
            .memory()
            .map(|(calls, _)| calls)
            .filter(|_| refresh.snapshot.memory_scope() == MemoryScope::Persona);
        let Some(calls) = moved else {
            let memory = self.tools.memory().map(|(calls, _)| calls);
            return self.tools.rehome(memory, shelves);
        };
        let Some(mut store) = self.store.take() else {
            return;
        };
        let name = persona.unwrap_or_else(|| NO_PERSONA.to_string());
        let connect: Rewire =
            Box::new(move |source, own| calls.moved(&personas, &name, &venue, (source, own)));
        let (store, wired) = blocking(move || {
            let wired = store.rewire(connect);
            (store, wired)
        })
        .await;
        self.store = Some(store);
        let calls = wired.unwrap_or_else(|error| {
            tracing::warn!(target: TARGET, error = %error, "memory not moved");
            None
        });
        if let Ok(mut room) = cell.write() {
            *room = calls.as_ref().map(|calls| calls.room().clone());
        }
        self.tools.rehome(calls, shelves);
    }
}
