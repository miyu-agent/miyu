//! 人经协议把别处的记忆导进来（施工 MI-1，`docs/blueprint/memory.md`「协议」的 `memory.import`）：旧版的迁移包照它把知识点
//! 导成新的记忆，保住原来记下的时刻。参数端点查过了；这里只管去重、记下，和记下以后的那几样。

use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_recall::{EPISODE, MemoryEvent, MemoryId, Origin, PERSON, Saved};
use miyu_tool::Refused;

use super::super::Using;
use super::super::her::{SELF_CLASS, is_her};
use super::super::landed::Landed;
use super::super::merge::{Hush, MergeJob};
use super::{Keeper, Stamp};
use crate::blocking::blocking;
use crate::config::TurnConfig;

/// 导进来的一条：端点照协议查过了（类、字数、是谁、关键词、`origin`）。
#[derive(Debug, Clone)]
pub struct Imported {
    /// 原来记下的时刻。
    pub at: Timestamp,
    /// 类。
    pub class: String,
    /// 正文。
    pub text: String,
    /// 在原来那边是哪一条：这一间导进来过的不再导。
    pub origin: String,
    /// 说的是谁：认识的人那一类才有。
    pub who: Option<String>,
    /// 涉及谁：发生过的事那一类才有。
    pub people: Vec<String>,
    /// 只用来检索的关键词；没有的没有。
    pub keywords: Option<String>,
}

impl Keeper {
    /// 照先后记下 `items`，交回每一条的编号：这一间导进来过同一个 `origin` 的（作废、改掉、清掉了的也算）跳过，是 `None`。
    /// `by` 照 `stamp`（人），出处空，听众是这次的听众（主人），原来的时刻记在 `noted`。认识的人那一类说的是她自己的，同
    /// [`Keeper::save`] 记成 `episode`、不写是谁。不认命令编号（`stamp.cause` 不写进去）：重发的照 `origin` 跳过。
    ///
    /// # Errors
    ///
    /// 记忆日志开不了、写不进：写进去了的照留，重发时照 `origin` 跳过。
    pub fn import(
        &self,
        stamp: Stamp,
        items: Vec<Imported>,
    ) -> Result<Vec<Option<MemoryId>>, Refused> {
        let log = self.log()?;
        let me = self.her();
        let (audience, venue) = self.listener.heard();
        let mut ids = Vec::with_capacity(items.len());
        for item in items {
            if log.book(|book| book.imported(&item.origin)) {
                ids.push(None);
                continue;
            }
            let who = item.who.filter(|_| item.class == PERSON);
            let (class, who) = match &who {
                Some(name) if is_her(&me, name) => (SELF_CLASS.to_string(), None),
                _ => (item.class, who),
            };
            let people = match class == EPISODE {
                true => item.people,
                false => Vec::new(),
            };
            let saved = Saved {
                class,
                text: item.text,
                sources: Vec::new(),
                audience: audience.clone(),
                replaces: None,
                about: None,
                venue: venue.clone(),
                keywords: item.keywords,
                who,
                people,
                place: None,
                imported: Some(Box::new(Origin {
                    noted: item.at,
                    origin: item.origin,
                })),
            };
            let stamp = Stamp {
                at: stamp.at,
                by: stamp.by.clone(),
                cause: None,
            };
            let (id, _) = self.append(&log, stamp, &MemoryEvent::Saved(saved))?;
            ids.push(Some(id));
        }
        Ok(ids)
    }

    /// 导完以后（施工 MI-1）：在后台给记下的 `saved` 写关键词、补向量，再照 E-9 看一眼够不够整理（导进了一条以上就当有
    /// 新记的；从没整理过的这一间马上整理，整理过的满 `memory.merge_every` 才整理；不轮询）。核心没交抽取要的几样的只补
    /// 向量。照 `config` 发、`offset` 写日期。
    pub fn settle_imported(&self, saved: Vec<MemoryId>, config: TurnConfig, offset: UtcOffset) {
        if saved.is_empty() {
            return;
        }
        let keeper = self.clone();
        tokio::spawn(async move {
            let Some(extraction) = keeper.extraction() else {
                let using = Using { config };
                blocking(move || keeper.fill(&using)).await;
                return;
            };
            let landed = Landed {
                keeper: keeper.clone(),
                extraction: extraction.clone(),
                config: config.clone(),
                offset,
            };
            landed.settle(&saved).await;
            MergeJob {
                keeper: keeper.as_owner(),
                extraction,
                config,
                offset,
                hush: Hush::default(),
                from: None,
            }
            .start();
        });
    }
}
