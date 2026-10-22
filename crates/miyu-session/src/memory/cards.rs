//! 人经协议碰人卡（施工 E-1 下，`docs/blueprint/memory.md` 第七条第 15 款、「协议」）：列、改一张、作废一张。卡是整理合的，人
//! 改的另写一张（`by` 是这个人），作废的追加 `ext.memory.card_retired`；原文都在日志里。

use miyu_recall::merge::{CARD_CHARS, NAME_CHARS, NAMES};
use miyu_recall::{Card, CardRetired, MemoryEvent, PersonCard};
use miyu_tool::Refused;

use super::keeper::done;
use super::{Keeper, Stamp};

/// 人改一张卡没成的为什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardRefused {
    /// 是谁、正文空的，正文超过 [`CARD_CHARS`] 字，叫法空的、太长的、超过 [`NAMES`] 个。
    Bad,
    /// 没有这个人的卡（作废的时候）。
    NoSuch,
    /// 记忆日志开不了、写不进。
    Failed(Refused),
}

impl Keeper {
    /// 这一间的人卡，照是谁排；`forgotten` 的连作废的一起。整间清空以前的没有。
    ///
    /// # Errors
    ///
    /// 记忆日志开不了：英文的一句为什么。
    pub fn cards(&self, forgotten: bool) -> Result<Vec<PersonCard>, String> {
        let log = self.log().map_err(super::keeper::failed)?;
        Ok(log.book(|book| {
            book.cards()
                .filter(|card| forgotten || card.retired.is_none())
                .cloned()
                .collect()
        }))
    }

    /// 人写一张卡（改，或者给还没有卡的人写一张）：去掉前后空白、照规矩查；这条命令做过的不再写。
    ///
    /// # Errors
    ///
    /// 不合规矩（[`CardRefused::Bad`]）；记忆日志开不了、写不进。
    pub fn write_card(&self, stamp: Stamp, card: Card) -> Result<(), CardRefused> {
        let card = Card {
            who: card.who.trim().to_string(),
            text: card.text.trim().to_string(),
            names: card
                .names
                .iter()
                .map(|name| name.trim().to_string())
                .collect(),
        };
        let bad_name = |name: &String| name.is_empty() || name.chars().count() > NAME_CHARS;
        if card.who.is_empty()
            || card.text.is_empty()
            || card.text.chars().count() > CARD_CHARS
            || card.names.len() > NAMES
            || card.names.iter().any(bad_name)
        {
            return Err(CardRefused::Bad);
        }
        let log = self.log().map_err(CardRefused::Failed)?;
        if done(&log, &stamp).is_some() {
            return Ok(());
        }
        self.append(&log, stamp, &MemoryEvent::Card(card))
            .map(|_| ())
            .map_err(CardRefused::Failed)
    }

    /// 人作废 `who` 的卡：已经作废了的什么都不写；这条命令做过的不再写。
    ///
    /// # Errors
    ///
    /// 没有这个人的卡（[`CardRefused::NoSuch`]）；记忆日志开不了、写不进。
    pub fn retire_card(&self, stamp: Stamp, who: &str, why: String) -> Result<(), CardRefused> {
        let log = self.log().map_err(CardRefused::Failed)?;
        if done(&log, &stamp).is_some() {
            return Ok(());
        }
        let retired = log
            .book(|book| book.card(who).map(|card| card.retired.is_some()))
            .ok_or(CardRefused::NoSuch)?;
        if retired {
            return Ok(());
        }
        let gone = CardRetired {
            who: who.to_string(),
            why,
        };
        self.append(&log, stamp, &MemoryEvent::CardRetired(gone))
            .map(|_| ())
            .map_err(CardRefused::Failed)
    }
}
