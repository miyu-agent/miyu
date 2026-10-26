//! 合卡以前认人（施工 E-1 下，设计 42 PN2）：她当场用 `remember` 记的认识的人，`who` 照她写的存，群里多半是群名片，不是平台
//! 编号。整理时照出处那几轮所在会话的日志认出这个名字对着谁（同抽取的 `Voices::names`：人话的场所名字、平台编号对着他的平台
//! 编号），这一批里的、交回来的卡都照平台编号算；对不上的照名字留着。抽取记的本来就是平台编号（施工 E-1 上），不用认。
//!
//! 只认不改：记下的那几条照旧是她写的名字（日志只追加），认出来的名字记进他那张卡的叫法里。
//!
//! 她自己不是认识的人（施工 E-1 修，`her.rs`）：`who` 是这一间她的名字的不算谁（不合卡、不认成同名群友的编号），她的名字也
//! 不拿来认人、不进谁的叫法。以前记下的这种照旧留着，只是不再合进卡。

use std::collections::{BTreeMap, BTreeSet};

use miyu_kernel::id::SessionId;
use miyu_kernel::origin::By;
use miyu_recall::{Entry, PERSON};
use miyu_store::log::read_events;
use miyu_store::recall::Room;

use crate::TARGET;
use crate::memory::Keeper;
use crate::memory::extract::Voices;

/// 这一批里认出来的人：她记的名字对着的平台编号。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct People {
    /// 名字到平台编号。
    known: BTreeMap<String, String>,
    /// 这一间她叫什么（照 `fold` 过的）。
    me: BTreeSet<String>,
}

impl People {
    /// `entry` 说的是谁，认出了的换成平台编号；不是认识的人那一类的没有。
    pub(super) fn who(&self, entry: &Entry) -> Option<String> {
        if entry.class != PERSON {
            return None;
        }
        let who = entry.who.as_deref()?;
        if crate::memory::her::is_her(&self.me, who) {
            return None;
        }
        Some(
            self.known
                .get(who)
                .cloned()
                .unwrap_or_else(|| who.to_string()),
        )
    }

    /// 平台编号 `id` 在这一批里被认出来的那几个名字：记进他的卡当叫法。
    pub(super) fn names_of(&self, id: &str) -> Vec<String> {
        self.known
            .iter()
            .filter(|(_, known)| known.as_str() == id)
            .map(|(name, _)| name.clone())
            .collect()
    }
}

impl Keeper {
    /// 认 `entries` 里的人（模块文档）：只读她当场记的（`by` 是工具）那几条的出处所在的会话，一个会话读一次；读不了的跳过、记
    /// 一行。在阻塞线程里调。
    pub(super) fn people(&self, entries: &[&Entry]) -> People {
        let sessions: BTreeSet<&SessionId> = entries
            .iter()
            .filter(|entry| entry.class == PERSON && matches!(entry.by, By::Tool(_)))
            .flat_map(|entry| entry.sources.iter().map(|source| &source.session))
            .collect();
        let me = self.her();
        let mut known = BTreeMap::new();
        for session in sessions {
            let Some(dir) = self.session_dir(session) else {
                continue;
            };
            match read_events(&dir) {
                Ok(events) => {
                    for (name, id) in Voices::of(&events, None).names {
                        if let Some(id) = id
                            && id != name
                            && !crate::memory::her::is_her(&me, &name)
                        {
                            known.insert(name, id);
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(target: TARGET, error = %error, "memory people not read");
                }
            }
        }
        People { known, me }
    }

    /// `people` 这几个人现在的卡（没作废的），照是谁排：合他们的卡时接着写。记忆日志开不了的没有。
    pub(super) fn cards_of(&self, people: &BTreeSet<String>) -> Vec<(String, String)> {
        let Ok(log) = self.log() else {
            return Vec::new();
        };
        log.book(|book| {
            people
                .iter()
                .filter_map(|who| book.card(who))
                .filter(|card| card.retired.is_none())
                .map(|card| (card.who.clone(), card.text.clone()))
                .collect()
        })
    }

    /// 会话 `session` 的目录：先看这一间的账号名下，没有再找别的账号（系统账号的会话、记忆归管理员，同出处活不活的判法）。
    fn session_dir(&self, session: &SessionId) -> Option<std::path::PathBuf> {
        let account = match self.room() {
            Room::Persona { account, .. } | Room::Session { account, .. } => account,
        };
        let root = &self.memory.root;
        let here = root.session_dir(account, session);
        if here.is_dir() {
            return Some(here);
        }
        let owner = root.owner_of(session).ok().flatten()?;
        Some(root.session_dir(&owner, session))
    }
}
