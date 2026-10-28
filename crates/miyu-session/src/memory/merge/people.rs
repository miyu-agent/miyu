//! 合卡以前认人（施工 E-1 下，设计 42 PN2）：她当场用 `remember` 记的认识的人，`who` 照她写的存，群里多半是群名片，不是平台
//! 编号。整理时照出处那几轮所在会话的日志认出这个名字对着谁（同抽取的 `Voices::names`：人话的场所名字、平台编号对着他的平台
//! 编号），这一批里的、交回来的卡都照平台编号算；对不上的照名字留着。抽取记的本来就是平台编号（施工 E-1 上），不用认。
//!
//! 只认不改：记下的那几条照旧是她写的名字（日志只追加），认出来的名字记进他那张卡的叫法里。
//!
//! 叫法核心认（施工 E-1 修二，第七条第 15 款）：这一批里认识的人那几条（抽取记的、她当场记的都算）的出处所在的会话，照日志
//! 认出每个平台编号在场所里真用过的名字（`venue.name`），最近用的在前，接在卡的叫法前面；模型交回的叫法只留出处里见过的
//! （真用过的、他那几条记忆的正文里写着的），编的不要（实测交回过源消息里从没出现过的名字，有的又空着、少了改名以前的）。
//!
//! 相处的事也算（施工 E-1 修四）：发生过的事（`episode`）带着涉及谁（`Entry::people`，抽取照说话人认过的平台编号或名字），这
//! 一批里有他的那几条也合进他的卡；认法同上（她记的名字照出处认、她自己不算），叫法照它们的出处一起认。
//!
//! 她自己不是认识的人（施工 E-1 修，`her.rs`）：`who` 是这一间她的名字的不算谁（不合卡、不认成同名群友的编号），她的名字也
//! 不拿来认人、不进谁的叫法。以前记下的这种照旧留着，只是不再合进卡。她的名字除了人格的，还有出处那几轮的会话日志里她在场所
//! 里被叫过的（`venue.my_name`，施工 E-1 修三）。

use std::collections::{BTreeMap, BTreeSet};

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::SessionId;
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;
use miyu_recall::{EPISODE, Entry, PERSON};
use miyu_store::log::read_events;

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
    /// 平台编号到他在出处里真用过的名字，最近用的在前（施工 E-1 修二）。
    used: BTreeMap<String, Vec<String>>,
    /// 是谁（认过的）到这一批里说的就是他的那几条记忆的正文（施工 E-1 修二）：交回的叫法写在里面的也算见过。认识的人那一类照
    /// 是谁；发生过的事只算只涉及他一个人的（施工 E-1 修五：一条带着两个人，正文里写着两个人的名字，算进去两人的名字就互串进
    /// 对方的卡）。
    said: BTreeMap<String, Vec<String>>,
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

    /// `entry` 涉及的认识的人（施工 E-1 修四）：发生过的事那一类的 `people`，认法同 [`People::who`]，去重；别的类没有。
    pub(super) fn involved(&self, entry: &Entry) -> Vec<String> {
        if entry.class != EPISODE {
            return Vec::new();
        }
        let mut found: Vec<String> = Vec::new();
        for name in &entry.people {
            if crate::memory::her::is_her(&self.me, name) {
                continue;
            }
            let one = self
                .known
                .get(name)
                .cloned()
                .unwrap_or_else(|| name.clone());
            if !found.contains(&one) {
                found.push(one);
            }
        }
        found
    }

    /// `entry` 说的是谁、涉及谁（施工 E-1 修四）：认识的人那一类的是谁，发生过的事涉及的几个人。
    pub(super) fn about(&self, entry: &Entry) -> Vec<String> {
        match self.who(entry) {
            Some(who) => vec![who],
            None => self.involved(entry),
        }
    }

    /// 平台编号 `id` 在这一批里被认出来的那几个名字：出处里真用过的（最近用的在前），再接她记的名字认出是他的。记进他的卡当
    /// 叫法。
    pub(super) fn names_of(&self, id: &str) -> Vec<String> {
        let mut names = self.used.get(id).cloned().unwrap_or_default();
        for (name, known) in &self.known {
            if known.as_str() == id && !names.contains(name) {
                names.push(name.clone());
            }
        }
        names
    }

    /// 交回的叫法 `name` 出处里见过没有（施工 E-1 修二）：`who` 在出处里用过的、她记的名字认出是他的、说的就是他的那几条记忆
    /// 的正文里写着的（见 `said`）。别人在出处里用过的名字不算（施工 E-1 修五）：正文里提到别人，不能把别人的名片认进他的卡。
    pub(super) fn seen(&self, who: &str, name: &str) -> bool {
        if self.names_of(who).iter().any(|known| known == name) {
            return true;
        }
        // 她记的名字认出是谁也是照出处里用过的名字认的（`known` 都在 `used` 里），只查 `used` 就够。
        let someone_else = self
            .used
            .iter()
            .filter(|(id, _)| id.as_str() != who)
            .any(|(_, names)| names.iter().any(|known| known == name));
        !someone_else
            && self
                .said
                .get(who)
                .is_some_and(|texts| texts.iter().any(|text| text.contains(name)))
    }

    /// 记下这一批里每个人那几条记忆的正文（施工 E-1 修二）：认识的人那一类照认过的是谁；发生过的事只记只涉及一个人的（施工 E-1
    /// 修五：带着几个人的那条正文里写着别人的名字，拿来认叫法会把一个人的名片认进另一个人的卡）。
    pub(super) fn note(&mut self, entries: &[Entry]) {
        for entry in entries {
            let about = self.about(entry);
            if let [who] = about.as_slice() {
                self.said
                    .entry(who.clone())
                    .or_default()
                    .push(entry.text.clone());
            }
        }
    }
}

impl Keeper {
    /// 认 `entries` 里的人（模块文档）：认识的人那几条的出处所在的会话，一个会话读一次；读不了的跳过、记一行。她记的名字只照
    /// 她当场记的（`by` 是工具）那几条的会话认，真用过的名字照全部那几条的会话认。在阻塞线程里调。
    pub(super) fn people(&self, entries: &[&Entry]) -> People {
        let sessions_of = |tool: bool| -> BTreeSet<&SessionId> {
            entries
                .iter()
                .filter(|entry| {
                    // 发生过的事带着涉及谁的也读（施工 E-1 修四）：它们的出处里也有这些人用过的名字。
                    let about = entry.class == PERSON
                        || (entry.class == EPISODE && !entry.people.is_empty());
                    about && (!tool || matches!(entry.by, By::Tool(_)))
                })
                .flat_map(|entry| entry.sources.iter().map(|source| &source.session))
                .collect()
        };
        let by_her = sessions_of(true);
        let mut me = self.her();
        let mut read: Vec<(&SessionId, Vec<Event>)> = Vec::new();
        for session in sessions_of(false) {
            let Some(dir) = self.session_dir(session) else {
                continue;
            };
            match read_events(&dir) {
                Ok(events) => {
                    // 她在出处里被叫过的名字也算她（施工 E-1 修三）：先全读完再认人。
                    me.extend(crate::memory::her::called_in(&events));
                    read.push((session, events));
                }
                Err(error) => {
                    tracing::warn!(target: TARGET, error = %error, "memory people not read");
                }
            }
        }
        let mut known = BTreeMap::new();
        let mut last: BTreeMap<String, BTreeMap<String, Timestamp>> = BTreeMap::new();
        for (session, events) in &read {
            used_in(events, &me, &mut last);
            if !by_her.contains(session) {
                continue;
            }
            for (name, id) in Voices::of(events, None).names {
                if let Some(id) = id
                    && id != name
                    && !crate::memory::her::is_her(&me, &name)
                {
                    known.insert(name, id);
                }
            }
        }
        let used = last
            .into_iter()
            .map(|(id, names)| {
                let mut names: Vec<(String, Timestamp)> = names.into_iter().collect();
                names.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
                (id, names.into_iter().map(|(name, _)| name).collect())
            })
            .collect();
        People {
            known,
            me,
            used,
            said: BTreeMap::new(),
        }
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

    /// 会话 `session` 的目录；不在的没有。
    fn session_dir(&self, session: &SessionId) -> Option<std::path::PathBuf> {
        let here = self.memory.root.session_dir(session);
        here.is_dir().then_some(here)
    }
}

/// 会话日志 `events` 里平台上的人在场所里用过的名字（`venue.name`），记每个名字最后一次用的时刻（施工 E-1 修二）。是她的名字的、
/// 和编号一样的不算。
fn used_in(
    events: &[Event],
    me: &BTreeSet<String>,
    last: &mut BTreeMap<String, BTreeMap<String, Timestamp>>,
) {
    for event in events {
        let (Body::MessageUser(message), By::External(external)) = (&event.body, &event.by) else {
            continue;
        };
        let Some(name) = message
            .venue
            .as_ref()
            .and_then(|venue| venue.name.as_deref())
        else {
            continue;
        };
        let (id, name) = (external.id.as_str(), name.trim());
        if name.is_empty() || name == id || crate::memory::her::is_her(me, name) {
            continue;
        }
        let at = last
            .entry(id.to_string())
            .or_default()
            .entry(name.to_string())
            .or_insert(event.at);
        if event.at > *at {
            *at = event.at;
        }
    }
}
