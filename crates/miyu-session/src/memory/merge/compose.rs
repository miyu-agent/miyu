//! 拼合并的那一次请求（施工 R-7 上；R-19 从 `merge.rs` 挪出来、加上顺带重看的旧记忆）：新记的照先后一条条加，每条带上相关的
//! 旧记忆；一趟里头一个放得下的批再接上轮到的旧记忆（`memory.md` 第七条第 11 款）；放不下的留给下一次。这一批里有认识的人
//! 的，最后接上合他们的卡的那一段（施工 E-1 下，第七条第 15 款）。

use std::collections::BTreeSet;
use std::sync::Arc;

use miyu_kernel::id::Seq;
use miyu_recall::{Entry, MemoryId, Swept};
use miyu_tool::Line;
use miyu_tool::load::say;

use super::super::Using;
use super::super::extract::ROOM;
use super::{Asked, Batch, MergeJob, RELATED};
use crate::TARGET;
use crate::blocking::blocking;

impl MergeJob {
    /// 拼这一次：新记的照先后一条条加，每条带上相关的旧记忆，放不下的留给下一次（一条都放不下也至少交一条）。这一批里有认识的
    /// 人的（施工 E-1 下），认出她记的名字是谁，最后接上合他们的卡的那一段和他们现在的卡。
    pub(super) async fn compose(&self, batch: Batch) -> Asked {
        let texts = &self.extraction.merge;
        let row = |entry: &Entry, about: &[String]| {
            texts.lines.say(&Line {
                id: &entry.id.to_string(),
                class: &marked(entry, about),
                date: &entry.at.local_date(self.offset),
                text: &entry.text.replace('\n', " "),
                venue: entry.venue.as_ref(),
                place: entry.place.as_deref(),
                retired: false,
            })
        };
        let summary = match &batch.summary {
            Some(text) => say(&texts.summary, &[("text", text)]),
            None => say(&texts.none, &[]),
        };
        // 摘要从主人写起：照属主的称呼（这一份配置就是属主的），不看是哪个会话起的合并（施工 R-6 再修）。
        let owner = super::super::owner::owner(
            super::super::called(&self.config).as_deref(),
            &texts.unnamed,
        );
        let head = format!(
            "{}\n\n{}",
            say(&texts.instruction, &[("owner", &owner)]).trim_end(),
            summary.trim_end(),
        );
        let new_head = say(&texts.new, &[]).trim_end().to_string();
        let related_head = say(&texts.related, &[]).trim_end().to_string();
        let sweep_head = say(&texts.sweep, &[]).trim_end().to_string();
        let fresh_ids: BTreeSet<MemoryId> = batch.fresh.iter().map(|entry| entry.id).collect();
        let (mut news, mut olds) = (Vec::new(), Vec::new());
        let mut given = BTreeSet::new();
        let mut bytes = head.len() + new_head.len() + related_head.len() + sweep_head.len() + 6;
        let mut upto = None;
        for entry in &batch.fresh {
            let related = self.related(entry, &fresh_ids, &given).await;
            let added: usize = std::iter::once(entry)
                .chain(&related)
                .map(|one| row(one, &[]).len() + 1)
                .sum();
            if upto.is_some() && bytes + added > ROOM {
                break;
            }
            bytes += added;
            news.push(entry.clone());
            given.insert(entry.id);
            for one in related {
                given.insert(one.id);
                olds.push(one);
            }
            upto = Some(entry.id.seq());
        }
        // 顺带重看的（施工 R-19）：照编号接在后面，已经交进去的不重复，放不下的这一趟不看了。
        let mut sweeps = Vec::new();
        let mut swept = None;
        for entry in &batch.sweep {
            // 已经当相关的交进去的也算看过了：轮到的照样往前走。
            if given.contains(&entry.id) {
                swept = Some(entry.id.seq());
                continue;
            }
            let added = row(entry, &[]).len() + 1;
            // 只重看的一批至少交一条，同新记的。
            if bytes + added > ROOM && !(news.is_empty() && sweeps.is_empty()) {
                break;
            }
            bytes += added;
            sweeps.push(entry.clone());
            given.insert(entry.id);
            swept = Some(entry.id.seq());
        }
        // 认识的人（施工 E-1 下）：认出她记的名字是谁，照平台编号列；发生过的事涉及的人也算（施工 E-1 修四）。再拿他们现在的卡。
        let all: Vec<Entry> = news.iter().chain(&olds).chain(&sweeps).cloned().collect();
        let keeper = self.keeper.clone();
        let (known, people, cards) = blocking(move || {
            let mut known = keeper.people(&all.iter().collect::<Vec<_>>());
            known.note(&all);
            let people: BTreeSet<String> =
                all.iter().flat_map(|entry| known.about(entry)).collect();
            let cards = keeper.cards_of(&people);
            (known, people, cards)
        })
        .await;
        // 只重看的一批合到哪照旧：上次合到的那一条。
        let upto = upto.or(batch.after).unwrap_or(Seq::FIRST);
        let mut text = head;
        for (title, entries) in [
            (&new_head, &news),
            (&related_head, &olds),
            (&sweep_head, &sweeps),
        ] {
            if !entries.is_empty() {
                let rows: Vec<String> = entries
                    .iter()
                    .map(|entry| row(entry, &known.about(entry)))
                    .collect();
                text.push_str(&format!("\n\n{title}\n{}", rows.join("\n")));
            }
        }
        if !people.is_empty() {
            let named = people
                .iter()
                .map(|who| listed(who, &known.names_of(who)))
                .collect::<Vec<_>>()
                .join(", ");
            text.push_str(&format!(
                "\n\n{}",
                say(&texts.people, &[("people", &named)]).trim_end()
            ));
            if !cards.is_empty() {
                let rows: Vec<String> = cards
                    .iter()
                    .map(|(who, card)| {
                        say(
                            &texts.card,
                            &[("who", who), ("text", &card.replace('\n', " "))],
                        )
                    })
                    .collect();
                let title = say(&texts.cards, &[]);
                text.push_str(&format!("\n{}\n{}", title.trim_end(), rows.join("\n")));
            }
        }
        let fresh = u32::try_from(news.len()).unwrap_or(u32::MAX);
        Asked {
            text,
            given,
            upto,
            fresh,
            swept: swept.map(|upto| Swept {
                upto,
                count: u32::try_from(sweeps.len()).unwrap_or(u32::MAX),
            }),
            people,
            known,
        }
    }

    /// `entry` 的相关旧记忆：照关键词和意思搜（有向量那一路的），不算新记的、已经交进去的，最多 [`RELATED`] 条。
    async fn related(
        &self,
        entry: &Entry,
        fresh: &BTreeSet<MemoryId>,
        given: &BTreeSet<MemoryId>,
    ) -> Vec<Entry> {
        let near = match self.keeper.vectors() {
            Some(vectors) => {
                let using = Using {
                    config: Arc::clone(&self.config),
                };
                vectors.query(&using, &entry.text).await
            }
            None => None,
        };
        let (keeper, text) = (self.keeper.clone(), entry.text.clone());
        let limit = RELATED + fresh.len();
        let found = blocking(move || keeper.search(&text, false, limit, near.as_ref())).await;
        match found {
            Ok(found) => found
                .into_iter()
                .filter(|one| !fresh.contains(&one.id) && !given.contains(&one.id))
                .take(RELATED)
                .collect(),
            Err(error) => {
                tracing::warn!(target: TARGET, room_kind = self.keeper.room().kind(), room_id = self.keeper.room_id().as_str(), error = error.as_str(), "memory merge search failed");
                Vec::new()
            }
        }
    }
}

/// 合卡那一段里列的一个人（施工 E-1 修六）：是谁后面接他在出处里用过的名字（[`super::people::People::names_of`]，最近用的在
/// 前，同卡的叫法去重、最多 [`miyu_recall::merge::NAMES`] 个），写法同搜到的卡（`qq:10086 (小林, 林林)`）；没有名字的只写是谁。
/// 记忆正文里写的是名片，带着说明的（「我不是 X」「本群最大 baka」）模型分不清哪截是名字，列出来才对得上（2026-10-27 主人旧
/// 主群真数据离线比：带说明名片的人卡 48% 把名片里的话写成他的事，列出来加一句以后 5%）。
fn listed(who: &str, names: &[String]) -> String {
    let names = miyu_recall::merge::names(who, names, &[]);
    match names.is_empty() {
        true => who.to_string(),
        false => format!("{who} ({})", names.join(", ")),
    }
}

/// 一条在合并的请求里的类：认识的人那一类后面接是谁（同 [`miyu_recall::shown_class`]，`about` 是认过的，空的照记下的），发生过的
/// 事涉及谁的后面接 `with` 和那几个人（施工 E-1 修四：合卡时看得出这件事是谁的），别的照类。
fn marked(entry: &Entry, about: &[String]) -> String {
    match (entry.class.as_str(), about) {
        (miyu_recall::EPISODE, [_, ..]) => format!("{} with {}", entry.class, about.join(", ")),
        (_, [who, ..]) => miyu_recall::shown_class(&entry.class, Some(who)),
        _ => miyu_recall::shown_class(&entry.class, entry.who.as_deref()),
    }
}
