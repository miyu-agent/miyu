//! 记下了几条以后（施工 MI-1 从抽取的那一件活挪出来，`docs/blueprint/memory.md` 第二条、第六条第 8 款）：给刚记下的写关键词、
//! 补向量。抽取记下以后、人经协议导进来以后（`memory.import`）都照它；之后看一眼够不够整理是调的一方的事（`MergeJob`）。

use std::sync::Arc;

use miyu_kernel::time::UtcOffset;
use miyu_recall::MemoryId;

use super::tag::{BATCH, Tagging};
use super::{Extraction, Keeper, Using};
use crate::TARGET;
use crate::blocking::blocking;
use crate::config::TurnConfig;

/// 记下了几条以后要的几样：哪一间、抽取那一次的（字、一次性入口、key 的写法）、这一轮的配置、会话的时区。
pub(crate) struct Landed {
    pub(crate) keeper: Keeper,
    pub(crate) extraction: Extraction,
    pub(crate) config: TurnConfig,
    pub(crate) offset: UtcOffset,
}

impl Landed {
    /// 给刚记下的 `saved` 写关键词（施工 R-15 下：一次最多 [`BATCH`] 条，多的接着分几次），再补向量（施工 R-5 五补：写上了
    /// 的照新的字补，只补一遍）。
    pub(crate) async fn settle(&self, saved: &[MemoryId]) {
        for some in saved.chunks(BATCH) {
            self.tag(some).await;
        }
        self.fill().await;
    }

    /// 给 `saved` 这几条写关键词：只写它们，补不满的不去拿别的（旧的整理时补）。
    async fn tag(&self, saved: &[MemoryId]) {
        let keeper = self.keeper.clone();
        let first = saved.to_vec();
        let limit = saved.len();
        let entries = match blocking(move || keeper.untagged(&first, limit)).await {
            Ok(entries) => entries,
            Err(refused) => {
                tracing::warn!(target: TARGET, room_kind = self.keeper.room().kind(), room_id = self.keeper.room_id().as_str(), error = ?refused, "memory tagging not read");
                return;
            }
        };
        let entries = entries
            .into_iter()
            .filter(|entry| saved.contains(&entry.id))
            .collect();
        let tagging = Tagging {
            keeper: self.keeper.clone(),
            extraction: self.extraction.clone(),
            config: Arc::clone(&self.config),
            offset: self.offset,
        };
        tagging.run(entries).await;
    }

    /// 在后台补这一间缺的向量（施工 R-5 五补：不等下一次搜）。
    async fn fill(&self) {
        let keeper = self.keeper.clone();
        let using = Using {
            config: Arc::clone(&self.config),
        };
        blocking(move || keeper.fill(&using)).await;
    }
}
