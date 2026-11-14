//! 扩展驱动报的「真发出去的字节」对不对得上（施工 MD-3，设计 66 第七节第 3 条）：线上的字节是扩展编的，核心看不见，只能记、查。同一个
//! 请求（规范字节的哈希）第二次出现（重试、重放对照），扩展报的 `sent` 却和上一次不一样的，说明它的编码看了参数以外的东西（时间、
//! 随机数、字段先后不固定），缓存前缀命中不了：调的一方记一行 `WARN`。每个驱动每次核心运行只说一次，免得刷屏。
//!
//! 每个驱动只留最近 [`KEPT`] 次（请求的哈希、报的 `sent`），够对上重试和重放，不跟着会话长大。
//!
//! 扩展驱动别的「只说一次」也记在这里（施工 HT-4）：交上来的供应商侧工具不在清单 `hosted` 里的，同一个驱动同一个名字每次核心
//! 运行只说一次。名字是扩展交的，扩展能无限造新名字，表得有上限：每个驱动最多记 [`UNDECLARED_PER_DRIVER`] 个名字，长过
//! [`HOSTED_NAME_BYTES`] 个字节的不进表；表满了又来新的、或者名字太长的，这个驱动只再说一次「还有更多」，之后都不说。

use std::collections::{BTreeSet, HashMap, VecDeque};

use miyu_config::package::HOSTED_NAME_BYTES;
use miyu_kernel::id::ContentHash;

/// 每个驱动留几次。
const KEPT: usize = 64;

/// 每个驱动最多记几个说过的没声明的名字。名字是扩展交的，不设上限的话扩展造一个新名字表就长一格，能撑大核心的内存；供应商的
/// 工具就那几件，正经的驱动远到不了这个数，到了的多半是扩展出了毛病，再往下一个个列也没用。
pub(super) const UNDECLARED_PER_DRIVER: usize = 64;

/// 驱动交了没声明的供应商侧工具，这一回说不说、说哪一行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Undeclared {
    /// 头一回见这个名字，进了表：记一行带名字的 `WARN`。
    Tell,
    /// 表里有，说过了：不记。
    Told,
    /// 这个驱动的表满了又来新名字，或者名字太长不进表，头一回：记一行「还有更多」的 `WARN`，不带名字。
    TellMore,
    /// 「还有更多」说过了：不记。
    ToldMore,
}

/// 一个驱动说过的没声明的名字。
#[derive(Debug, Default)]
struct Unlisted {
    /// 说过的名字，最多 [`UNDECLARED_PER_DRIVER`] 个。
    names: BTreeSet<String>,
    /// 「还有更多」说过了。
    more: bool,
}

/// 各驱动最近几次的请求和报的 `sent`，和说过的驱动。
#[derive(Debug, Default)]
pub(crate) struct Wires {
    seen: HashMap<String, VecDeque<(ContentHash, ContentHash)>>,
    warned: BTreeSet<String>,
    /// 各驱动说过的没声明的供应商侧工具。
    undeclared: HashMap<String, Unlisted>,
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

    /// 驱动 `driver` 交了没声明的供应商侧工具 `name`：交回这一回说不说、说哪一行（调的一方照它记 `WARN`，见模块的说明）。
    pub(crate) fn undeclared(&mut self, driver: &str, name: &str) -> Undeclared {
        let unlisted = self.undeclared.entry(driver.to_string()).or_default();
        if unlisted.names.contains(name) {
            Undeclared::Told
        } else if name.len() <= HOSTED_NAME_BYTES && unlisted.names.len() < UNDECLARED_PER_DRIVER {
            unlisted.names.insert(name.to_string());
            Undeclared::Tell
        } else if std::mem::replace(&mut unlisted.more, true) {
            Undeclared::ToldMore
        } else {
            Undeclared::TellMore
        }
    }
}

#[cfg(test)]
mod tests;
