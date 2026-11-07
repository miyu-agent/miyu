//! 扩展驱动报的「真发出去的字节」对不对得上（施工 MD-3，设计 66 第七节第 3 条）：线上的字节是扩展编的，核心看不见，只能记、查。同一个
//! 请求（规范字节的哈希）第二次出现（重试、重放对照），扩展报的 `sent` 却和上一次不一样的，说明它的编码看了参数以外的东西（时间、
//! 随机数、字段先后不固定），缓存前缀命中不了：调的一方记一行 `WARN`。每个驱动每次核心运行只说一次，免得刷屏。
//!
//! 每个驱动只留最近 [`KEPT`] 次（请求的哈希、报的 `sent`），够对上重试和重放，不跟着会话长大。

use std::collections::{BTreeSet, HashMap, VecDeque};

use miyu_kernel::id::ContentHash;

/// 每个驱动留几次。
const KEPT: usize = 64;

/// 各驱动最近几次的请求和报的 `sent`，和说过的驱动。
#[derive(Debug, Default)]
pub(crate) struct Wires {
    seen: HashMap<String, VecDeque<(ContentHash, ContentHash)>>,
    warned: BTreeSet<String>,
}

impl Wires {
    /// 驱动 `driver` 把请求 `request` 编成了 `sent`：同一个请求以前编出过不同的字节、这个驱动这次运行还没说过的，交回以前的那一份
    /// （调的一方记一行 `WARN`）；别的没有。
    pub(crate) fn note(
        &mut self,
        driver: &str,
        request: &ContentHash,
        sent: &ContentHash,
    ) -> Option<ContentHash> {
        let kept = self.seen.entry(driver.to_string()).or_default();
        let before = kept
            .iter()
            .find(|(seen, _)| seen == request)
            .map(|(_, before)| before.clone());
        if before.is_none() {
            if kept.len() == KEPT {
                kept.pop_front();
            }
            kept.push_back((request.clone(), sent.clone()));
        }
        before.filter(|before| before != sent && self.warned.insert(driver.to_string()))
    }
}

#[cfg(test)]
mod tests;
