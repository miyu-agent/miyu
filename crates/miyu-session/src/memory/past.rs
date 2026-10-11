//! 搜以前的对话（施工 R-3 中，`docs/blueprint/memory.md` 第一条、「工具」）：查这一间的回合索引，她的 `memory_search` 和协议的
//! `memory.turns`（施工 R-12 下）共用；照谁在听挑（施工 R-13 下，第九条）：一个场所里只搜得到会话在这个场所的。

use std::collections::BTreeMap;

use miyu_kernel::id::{SessionId, TurnId};
use miyu_kernel::time::Timestamp;
use miyu_recall::fuse;
use miyu_tool::FoundTurn;

use super::Keeper;
use super::vectors::Query;

impl Keeper {
    /// 搜这一间的以前的对话（回合索引），最多 `limit` 段；`skip` 这个会话自己的不算（她看得见；人经协议搜的不跳过，施工
    /// R-12 下）；照谁在听挑（施工 R-13 下）：一个场所里只有会话在这个场所的。照关键词找；有问句的向量 `near` 的（施工 R-5 下）
    /// 再照意思找，两路照名次合。
    ///
    /// # Errors
    ///
    /// 回合库读不了。
    pub fn turns(
        &self,
        query: &str,
        limit: usize,
        skip: Option<&SessionId>,
        near: Option<&Query>,
    ) -> Result<Vec<FoundTurn>, String> {
        let (turns, _) = self.memory.turns.turns(self.room());
        let places = turns.places().map_err(|error| error.to_string())?;
        let own = skip.map(|skip| format!("{skip}/"));
        let wide = limit.saturating_mul(4);
        let hits = turns
            .search(query, wide)
            .map_err(|error| error.to_string())?;
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
        Ok(fuse(&keys, &similar)
            .into_iter()
            .filter(|key| own.as_ref().is_none_or(|own| !key.starts_with(own)))
            .filter(|key| {
                let source = key
                    .split_once('/')
                    .map_or(key.as_str(), |(source, _)| source);
                self.hears_place(places.get(source).map(String::as_str))
            })
            .filter_map(|key| {
                let (text, at) = found.remove(&key)?;
                let (session, turn) = key.split_once('/')?;
                let turn = turn.parse().ok().and_then(miyu_kernel::id::Seq::new)?;
                Some(FoundTurn {
                    session: SessionId::parse(session).ok()?,
                    turn: TurnId::new(turn),
                    at,
                    text,
                })
            })
            .take(limit)
            .collect())
    }
}
