//! 搜记下的（施工 R-3 补；R-14 加关键词那一路的下限，R-24 加只照意思那一路照模型的下限，从 `keeper.rs` 拆出来）。

use miyu_recall::{Entry, MemoryId, fuse_above};

use super::{Keeper, Query, failed};

impl Keeper {
    /// 搜记下的，最多 `limit` 条，最相关的在前：只给现在算数的（`forgotten` 时作废的也给）、出处活着的、听众合的。照关键词找；
    /// 有问句的向量 `near` 的（施工 R-5 下）再照意思找，两路照名次合（`miyu_recall::fuse`）。模型在关键词下限表里的（施工 R-14，
    /// `core/memory/search.toml`），只有关键词找到的那几条和问句不够像的不要，还没补上向量的照留；只照意思找到的照
    /// `core/memory/near.toml`（施工 R-24，表里没有的 0.40）。
    ///
    /// # Errors
    ///
    /// 记忆日志开不了、记忆库读不了。
    pub fn search(
        &self,
        query: &str,
        forgotten: bool,
        limit: usize,
        near: Option<&Query>,
    ) -> Result<Vec<Entry>, String> {
        let log = self.log().map_err(failed)?;
        // 多要几倍：改掉的、作废的、出处死了的、听众不合的挑掉以后还够。
        let wide = limit.saturating_mul(4);
        let mut ids = log.search(query, wide).map_err(|error| error.to_string())?;
        let recall = self.memory.recall();
        let floor =
            near.and_then(|near| Some((near, recall.as_ref()?.keyword_floor(&near.model)?)));
        if let Some((near, floor)) = floor {
            let keys: Vec<String> = ids.iter().map(ToString::to_string).collect();
            let similar = log
                .index()
                .similar_to(&near.model, &near.vector, &keys)
                .map_err(|error| error.to_string())?;
            ids.retain(|id| {
                similar
                    .get(&id.to_string())
                    .is_none_or(|similar| *similar >= floor)
            });
        }
        let close = match near {
            Some(near) => log
                .index()
                .nearest(&near.model, &near.vector, wide)
                .map_err(|error| error.to_string())?
                .into_iter()
                .filter_map(|hit| MemoryId::parse(&hit.key).map(|id| (id, hit.similar)))
                .collect(),
            None => Vec::new(),
        };
        let floor = match (near, &recall) {
            (Some(near), Some(recall)) => recall.near_floor(&near.model),
            _ => miyu_recall::FLOOR,
        };
        let ids = fuse_above(&ids, &close, floor);
        let found = log.book(|book| {
            ids.iter()
                .filter_map(|id| book.get(*id).cloned())
                .collect::<Vec<Entry>>()
        });
        Ok(found
            .into_iter()
            .filter(|entry| self.shown(entry, forgotten))
            .take(limit)
            .collect())
    }
}
