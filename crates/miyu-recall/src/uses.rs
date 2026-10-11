//! 想起过什么：一条记忆什么时候被用到（施工 R-4 下，`docs/blueprint/memory.md` 第八条）：照会话日志派生，不另存真相。
//!
//! - 用到一次：一条事件带上了这一条。模块 `memory` 注入的一块（常驻那一块、联想那一块）的 `refs` 里有它；她调
//!   `memory_search` 成功了、结果开头那几行列了它（`m12 user …`，到第一行不是记忆的为止：以前的对话那几行、
//!   `Nothing found.` 不算）。同一条事件里列了几遍算一次。
//! - [`UseFeed`] 是增量的，同回合索引的 `TurnFeed`：会话每落一批，一条条交给它；它只记还没等到结果的那几次搜索。
//! - 模块编号、工具名由调的一方交进来：工具名的常量在 `miyu-tool`，它在这一层上面。

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use miyu_kernel::block::{Block, words};
use miyu_kernel::event::{Body, Event, ToolStatus};
use miyu_kernel::id::{CallId, Seq};
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;

use crate::memory::MemoryId;

/// 用到一次。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Use {
    /// 会话日志里带上它的那一条事件。
    pub seq: Seq,
    /// 哪一条记忆。
    pub memory: MemoryId,
    /// 那一条事件的时刻。
    pub at: Timestamp,
}

/// 照一条条事件算用到了哪几条（增量）。
#[derive(Debug)]
pub struct UseFeed {
    /// 记忆这个模块的编号：它注入的一块的 `refs` 算。
    module: String,
    /// 搜记忆的工具名：它成了的结果算。
    search: String,
    /// 调了搜索、还没等到结果的那几次。
    calls: BTreeSet<CallId>,
}

impl UseFeed {
    /// 模块 `module` 注入的、工具 `search` 搜到的算用到。
    #[must_use]
    pub fn new(module: &str, search: &str) -> UseFeed {
        UseFeed {
            module: module.to_string(),
            search: search.to_string(),
            calls: BTreeSet::new(),
        }
    }

    /// 载入时整份过一遍：交回序号大于 `after` 的（照到的位置以后的；`None` 的整份交）。照到的位置以前调的搜索，结果在以后
    /// 的，照样认得。
    pub fn prime(&mut self, events: &[Event], after: Option<Seq>) -> Vec<Use> {
        let mut uses = Vec::new();
        for event in events {
            let seen = self.see(event);
            if after.is_none_or(|after| event.seq > after) {
                uses.extend(seen);
            }
        }
        uses
    }

    /// 看一条事件，交回它带上的那几条（照先后，不重）。
    pub fn see(&mut self, event: &Event) -> Vec<Use> {
        let ids = match &event.body {
            Body::ContextInjected(injected) if self.ours(&event.by) => injected
                .refs
                .iter()
                .filter_map(|reference| MemoryId::parse(reference))
                .collect(),
            Body::MessageAssistant(message) => {
                for block in &message.blocks {
                    if let Block::ToolCall(call) = block
                        && call.name == self.search
                    {
                        self.calls.insert(call.call_id);
                    }
                }
                return Vec::new();
            }
            Body::ToolResult(result) => {
                if !self.calls.remove(&result.call_id) || result.status != ToolStatus::Ok {
                    return Vec::new();
                }
                listed(&words(&result.blocks))
            }
            _ => return Vec::new(),
        };
        let mut seen = BTreeSet::new();
        ids.into_iter()
            .filter(|id| seen.insert(*id))
            .map(|memory| Use {
                seq: event.seq,
                memory,
                at: event.at,
            })
            .collect()
    }

    /// 是记忆这个模块注入的。
    fn ours(&self, by: &By) -> bool {
        matches!(by, By::Module(module) if module.id.as_str() == self.module)
    }
}

/// 搜到的字开头那几行的编号：一行一条记忆，头一个词是编号（`memory_search/memory.txt`、`retired.txt`），到第一行不是的
/// 为止。记忆的正文里的换行已经换成空格，以前的对话那几行在后面，不会被认成记忆。
fn listed(text: &str) -> Vec<MemoryId> {
    text.lines()
        .map_while(|line| {
            line.split_once(' ')
                .and_then(|(first, _)| MemoryId::parse(first))
        })
        .collect()
}
