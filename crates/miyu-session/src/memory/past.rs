//! 搜以前的对话（施工 R-3 中，`docs/blueprint/memory.md` 第一条、「工具」）：查这一间的回合索引，她的 `memory_search` 和协议的
//! `memory.turns`（施工 R-12 下）共用；哪儿都搜得到全部（施工 R-29，第九条）。

use std::collections::BTreeMap;

use miyu_kernel::id::{AccountId, Seq, SessionId, TurnId, VenueId};
use miyu_kernel::time::Timestamp;
use miyu_recall::fuse_above;
use miyu_store::recall::Origin;
use miyu_tool::FoundTurn;

use super::Keeper;
use super::vectors::Query;

impl Keeper {
    /// 搜这一间的以前的对话（回合索引），最多 `limit` 段；`seen` 是她这时看得见的：这个会话、还算数的最近一次压缩、清空替代
    /// 到哪，这以后开的轮还在上下文里、不算，以前的照常搜得到；没压缩、清空过的整个会话都不算（施工 R-13 补：一直开着的群会话
    /// 里清空掉的几轮原来搜不到）。人经协议搜的不跳过（施工 R-12 下）。哪儿都搜得到全部（施工 R-29：原来一个场所里只搜得到会话在
    /// 这个场所的）。照关键词找；有问句的向量 `near` 的（施工 R-5 下）再照意思找，两路照名次合。下限照模型（施工 R-20，
    /// `docs/blueprint/recall.md` 第三条第 3 款）：只照意思找到的照 `core/memory/turns.toml`（表里没有的 0.40），只照关键词找到的
    /// 和问句不够像的照记忆搜那张 `core/memory/search.toml` 不要（表里没有的不设，还没补上向量的照留）。每一段带上那个会话在哪个
    /// 场所、属主是 `asker` 的会话打不打得开它（施工 E-6，没有 `asker` 的一律打不开）。
    ///
    /// # Errors
    ///
    /// 回合库读不了。
    pub fn turns(
        &self,
        query: &str,
        limit: usize,
        (seen, asker): (Option<(&SessionId, Option<Seq>)>, Option<&AccountId>),
        near: Option<&Query>,
    ) -> Result<Vec<FoundTurn>, String> {
        self.memory.settle();
        let (turns, _) = self.memory.turns.turns(self.room());
        let places = turns.places().map_err(|error| error.to_string())?;
        let origins = turns.origins().map_err(|error| error.to_string())?;
        let own = seen.map(|(session, after)| (format!("{session}/"), after));
        let wide = limit.saturating_mul(4);
        let mut hits = turns
            .search(query, wide)
            .map_err(|error| error.to_string())?;
        let recall = self.memory.recall();
        if let Some((near, floor)) =
            near.and_then(|near| Some((near, recall.as_ref()?.keyword_floor(&near.model)?)))
        {
            let keys: Vec<String> = hits.iter().map(|hit| hit.key.clone()).collect();
            let similar = turns
                .similar_to(&near.model, &near.vector, &keys)
                .map_err(|error| error.to_string())?;
            hits.retain(|hit| {
                similar
                    .get(&hit.key)
                    .is_none_or(|similar| *similar >= floor)
            });
        }
        let floor = match (near, &recall) {
            (Some(near), Some(recall)) => recall.turns_floor(&near.model),
            _ => miyu_recall::FLOOR,
        };
        let close = match near {
            Some(near) => turns
                .nearest(&near.model, &near.vector, wide)
                .map_err(|error| error.to_string())?,
            None => Vec::new(),
        };
        let keys: Vec<String> = hits.iter().map(|hit| hit.key.clone()).collect();
        let similar: Vec<(String, f32)> = close
            .iter()
            .map(|hit| (hit.key.clone(), hit.similar))
            .collect();
        // 合完照键找回字和时刻：两路交回的是同一条的同一份。
        let mut found: BTreeMap<String, (String, Timestamp)> = close
            .into_iter()
            .map(|hit| (hit.key, (hit.text, hit.at)))
            .collect();
        found.extend(hits.into_iter().map(|hit| (hit.key, (hit.text, hit.at))));
        Ok(fuse_above(&keys, &similar, floor)
            .into_iter()
            .filter(|key| {
                own.as_ref().is_none_or(|(own, after)| {
                    key.strip_prefix(own.as_str())
                        .is_none_or(|turn| replaced(turn, *after))
                })
            })
            .filter_map(|key| {
                let (text, at) = found.remove(&key)?;
                let (session, turn) = key.split_once('/')?;
                let turn = turn.parse().ok().and_then(miyu_kernel::id::Seq::new)?;
                let venue = places
                    .get(session)
                    .and_then(|venue| VenueId::parse(venue).ok());
                let opens = asker.is_some_and(|asker| opens(asker, origins.get(session), &venue));
                Some(FoundTurn {
                    session: SessionId::parse(session).ok()?,
                    turn: TurnId::new(turn),
                    at,
                    text,
                    venue,
                    opens,
                })
            })
            .take(limit)
            .collect())
    }
}

/// 这个会话的第 `turn` 轮（键的后一半）是不是已经被压缩、清空替代掉了：回合编号不大于 `after` 的是。没有 `after` 的都还在。
fn replaced(turn: &str, after: Option<Seq>) -> bool {
    after.is_some_and(|after| turn.parse::<u64>().is_ok_and(|turn| turn <= after.get()))
}

/// 属主是 `asker` 的会话打不打得开这一段的会话（施工 E-6）：同 `history` 认编号的那一批（`SessionsPort::list`：属主一样的主
/// 会话）。回合库记着从哪来的照它判；没记过的（接上回合索引以前的库、还没载入过的会话）照场所判：场所会话打不开，本机的当是
/// 自己的主会话（原来的样子）。问的会话自己读不了别的会话的（群），工具照旧不给编号（`miyu-memory` 的 `search.rs`）。
fn opens(asker: &AccountId, origin: Option<&Origin>, venue: &Option<VenueId>) -> bool {
    match origin {
        Some(origin) => origin.main && origin.owner == asker.as_str(),
        None => venue.is_none(),
    }
}
