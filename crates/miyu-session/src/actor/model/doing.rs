//! 回合开始交「她现在在干嘛」（施工 LF-7，设计 `70-推演人生.md` 第三节第 8 条，`docs/blueprint/life.md`「现在在干嘛」）：同一个
//! 人格的人生会话里她写的那一句，变了才在这个会话的回合开头追加一块。
//!
//! - 哪些会话：带人格、不是派出去的子代理（`lineage`，人手分叉出来的支线不算子代理）。人生会话自己、人生停着的、她没写过的由
//!   端口判（`HooksPort::doing`）。
//! - 交不交：这一段上下文里最近一块同模块、同类别的记号（`refs`，写下的时刻）一样的不交；压缩、撤掉以后没有了的重交。
//! - 只查内存，不读盘，不另限时：端口交回写好的一行。

use std::sync::Arc;

use miyu_kernel::event::ContextInjected;
use miyu_kernel::facts::Present;
use miyu_kernel::id::{FactKind, ModuleId, SessionId};
use miyu_kernel::session::Injection;
use miyu_kernel::time::UtcOffset;
use miyu_tool::HooksPort;

/// 注入那一块的模块：`life`。
const MODULE: &str = "life";

/// 这一块的类别：`doing`。
const KIND: &str = "doing";

/// 这个会话这一轮开头要不要交、交什么：`port` 是回合开始的端口和这个会话的编号（没交端口的没有），`persona` 是会话这一刻的
/// 人格，`child` 是不是派出去的子代理，`offset` 是会话的时区，`present` 是这一段上下文里模块注入过的。
pub(super) fn doing(
    port: Option<(Arc<dyn HooksPort>, SessionId)>,
    (persona, child): (Option<&str>, bool),
    offset: UtcOffset,
    present: &[Present],
) -> Option<Injection> {
    if child {
        return None;
    }
    let (port, session) = port?;
    let line = port.doing(persona?, session.as_str(), offset)?;
    let last = present
        .iter()
        .rev()
        .find(|present| present.module.as_str() == MODULE && present.kind.as_str() == KIND);
    if last.is_some_and(|last| last.refs == [line.key.as_str()]) {
        return None;
    }
    Some(Injection {
        module: ModuleId::parse(MODULE).expect("合写法"),
        fact: ContextInjected {
            kind: FactKind::parse(KIND).expect("合写法"),
            text: line.text,
            refs: vec![line.key],
            after: false,
        },
    })
}

#[cfg(test)]
mod tests;
