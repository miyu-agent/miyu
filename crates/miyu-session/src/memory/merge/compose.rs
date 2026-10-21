//! 拼合并的那一次请求（施工 R-7 上；R-19 从 `merge.rs` 挪出来、加上顺带重看的旧记忆）：新记的照先后一条条加，每条带上相关的
//! 旧记忆；一趟里头一个放得下的批再接上轮到的旧记忆（`memory.md` 第七条第 11 款）；放不下的留给下一次。

use std::collections::BTreeSet;
use std::sync::Arc;

use miyu_kernel::id::Seq;
use miyu_recall::{Entry, MemoryId, Swept};
use miyu_tool::load::say;

use super::super::Using;
use super::super::extract::ROOM;
use super::{Asked, Batch, MergeJob, RELATED};
use crate::TARGET;
use crate::blocking::blocking;

impl MergeJob {
    /// 拼这一次：新记的照先后一条条加，每条带上相关的旧记忆，放不下的留给下一次（一条都放不下也至少交一条）。
    pub(super) async fn compose(&self, batch: Batch) -> Asked {
        let texts = &self.extraction.merge;
        let row = |entry: &Entry| {
            say(
                &texts.line,
                &[
                    ("id", &entry.id.to_string()),
                    (
                        "class",
                        &miyu_recall::shown_class(&entry.class, entry.who.as_deref()),
                    ),
                    ("date", &entry.at.local_date(self.offset)),
                    ("text", &entry.text.replace('\n', " ")),
                ],
            )
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
        let (mut new_rows, mut old_rows) = (Vec::new(), Vec::new());
        let mut given = BTreeSet::new();
        let mut bytes = head.len() + new_head.len() + related_head.len() + sweep_head.len() + 6;
        let mut upto = None;
        for entry in &batch.fresh {
            let related = self.related(entry, &fresh_ids, &given).await;
            let added: usize = std::iter::once(entry)
                .chain(&related)
                .map(|one| row(one).len() + 1)
                .sum();
            if upto.is_some() && bytes + added > ROOM {
                break;
            }
            bytes += added;
            new_rows.push(row(entry));
            given.insert(entry.id);
            for one in &related {
                old_rows.push(row(one));
                given.insert(one.id);
            }
            upto = Some(entry.id.seq());
        }
        // 顺带重看的（施工 R-19）：照编号接在后面，已经交进去的不重复，放不下的这一趟不看了。
        let mut sweep_rows = Vec::new();
        let mut swept = None;
        for entry in &batch.sweep {
            // 已经当相关的交进去的也算看过了：轮到的照样往前走。
            if given.contains(&entry.id) {
                swept = Some(entry.id.seq());
                continue;
            }
            let added = row(entry).len() + 1;
            // 只重看的一批至少交一条，同新记的。
            if bytes + added > ROOM && !(new_rows.is_empty() && sweep_rows.is_empty()) {
                break;
            }
            bytes += added;
            sweep_rows.push(row(entry));
            given.insert(entry.id);
            swept = Some(entry.id.seq());
        }
        // 只重看的一批（人叫的整理、没有新记的）合到哪照旧：上次合到的那一条。
        let upto = upto.or(batch.after).unwrap_or(Seq::FIRST);
        let mut text = head;
        for (title, rows) in [
            (&new_head, &new_rows),
            (&related_head, &old_rows),
            (&sweep_head, &sweep_rows),
        ] {
            if !rows.is_empty() {
                text.push_str(&format!("\n\n{title}\n{}", rows.join("\n")));
            }
        }
        let fresh = u32::try_from(new_rows.len()).unwrap_or(u32::MAX);
        Asked {
            text,
            given,
            upto,
            fresh,
            swept: swept.map(|upto| Swept {
                upto,
                count: u32::try_from(sweep_rows.len()).unwrap_or(u32::MAX),
            }),
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
                    owner: self.owner.clone(),
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
                tracing::warn!(target: TARGET, room = %self.keeper.room(), error = error.as_str(), "memory merge search failed");
                Vec::new()
            }
        }
    }
}
