//! 改一条（施工 R-3 补的 `memory.update`；R-15 上加只换关键词，从 `keeper.rs` 拆出来）。

use miyu_recall::{MemoryEvent, MemoryId, Tagged};
use miyu_tool::{Refused, Remember};

use super::{Keeper, Stamp, done};

impl Keeper {
    /// 改一条（协议的 `memory.update`）：给了 `text` 的新记一条 `replaces` 它，类照旧的，出处空，关键词照 `keywords`（没给的带上
    /// 旧的，空的是去掉）；只给了 `keywords` 的只换关键词，编号不变（施工 R-15 上）。两个都没给的照旧交回这一条，什么都不写。
    ///
    /// # Errors
    ///
    /// 同 [`Keeper::save`]。
    pub fn update(
        &self,
        stamp: Stamp,
        id: MemoryId,
        text: Option<String>,
        keywords: Option<String>,
    ) -> Result<MemoryId, Refused> {
        let log = self.log()?;
        if let Some((done, _)) = done(&log, &stamp) {
            return Ok(done);
        }
        let class = self.current(&log, id)?.class;
        let Some(text) = text else {
            if let Some(keywords) = keywords {
                self.append(&log, stamp, &MemoryEvent::Tagged(Tagged { id, keywords }))?;
            }
            return Ok(id);
        };
        let remember = Remember {
            class,
            text,
            replaces: Some(id),
            keywords,
        };
        self.save(stamp, remember, Vec::new())
    }
}
