//! 在跑的工具的进度（施工 X-45 补，`docs/blueprint/view.md` 的 `tool` 条目）：`tool.progress` 的字里一整行以
//! [`PREFIX`] 开头的，去了前缀、首尾空白，写进那一步的 `progress`。
//!
//! - 照调用编号攒半行：一段的末尾没有换行的留着等下一段。只留可能是进度行的：开头和前缀对不上的当场扔掉、丢到换行；
//!   最多留 [`HELD`] 个字节，内存不涨。
//! - 一段里几行取最后一条；空的不算；最长 [`LONGEST`] 个字；和现在的一样的不推。
//! - 合并：同一个调用离上次推不到 [`GAP_MS`] 的压着，[`Projector::due`] 交回最早到点的时刻，[`Projector::tick`] 推出去。
//!   时刻照事件的 `at`，和端点的钟是同一个。
//! - 结果到了（[`Projector::progress_over`]），这些都扔掉。

use std::collections::BTreeMap;

use miyu_kernel::event::ToolProgress;
use miyu_kernel::id::CallId;
use miyu_kernel::time::Timestamp;

use super::Projector;
use crate::change::Change;
use crate::entry::Body;

/// 进度行的开头（同 X-45 的脚本工具）：照字面，分大小写，后面带一个空格。
const PREFIX: &str = "miyu-progress: ";

/// 一行进度最长几个字（Unicode 字符），多的截掉。
const LONGEST: usize = 200;

/// 半行最多留几个字节：够前缀加 [`LONGEST`] 个四字节的字。
const HELD: usize = 1024;

/// 同一个调用两次推之间至少隔多少毫秒。
const GAP_MS: i64 = 500;

/// 每个在跑的调用的进度。
#[derive(Debug, Default)]
pub(super) struct Progress {
    calls: BTreeMap<CallId, Line>,
}

/// 一个调用的进度：攒着的半行，推出去的、压着的值，上次推的时刻。
#[derive(Debug, Default)]
struct Line {
    half: String,
    /// 这一行看出来不是进度行：丢到换行。
    skip: bool,
    shown: Option<String>,
    pending: Option<String>,
    pushed: Option<Timestamp>,
}

impl Line {
    /// 喂一段字，交回这一段里收全的最后一行进度。
    fn feed(&mut self, text: &str) -> Option<String> {
        let mut latest = None;
        let mut rest = text;
        while let Some(end) = rest.find('\n') {
            self.take(&rest[..end]);
            if let Some(value) = self.line_ends() {
                latest = Some(value);
            }
            rest = &rest[end + 1..];
        }
        self.take(rest);
        latest
    }

    /// 接上一截（不带换行）：开头和前缀对不上的丢到换行，留着的不过 [`HELD`]。
    fn take(&mut self, piece: &str) {
        if self.skip {
            return;
        }
        let mut room = HELD.saturating_sub(self.half.len()).min(piece.len());
        while !piece.is_char_boundary(room) {
            room -= 1;
        }
        self.half.push_str(&piece[..room]);
        let n = self.half.len().min(PREFIX.len());
        if self.half.as_bytes()[..n] != PREFIX.as_bytes()[..n] {
            self.skip = true;
            self.half.clear();
        }
    }

    /// 一行收全了：是进度行的交回它的值。
    fn line_ends(&mut self) -> Option<String> {
        let line = std::mem::take(&mut self.half);
        if std::mem::take(&mut self.skip) {
            return None;
        }
        let value = line.strip_prefix(PREFIX)?.trim();
        (!value.is_empty()).then(|| value.chars().take(LONGEST).collect())
    }

    /// 压着的到点的时刻。
    fn due(&self) -> Option<Timestamp> {
        self.pending.as_ref()?;
        Timestamp::from_unix_millis(self.pushed?.unix_millis() + GAP_MS)
    }
}

impl Projector {
    /// 一段进度：收全的最后一行，变了的离上次够 [`GAP_MS`] 就推，不够的压着。
    pub(super) fn tool_progress(&mut self, at: Timestamp, progress: &ToolProgress) {
        let line = self.progress.calls.entry(progress.call_id).or_default();
        let Some(value) = line.feed(&progress.text) else {
            return;
        };
        if line.shown.as_ref() == Some(&value) {
            line.pending = None;
            return;
        }
        match line.pushed {
            Some(pushed) if at.unix_millis() - pushed.unix_millis() < GAP_MS => {
                line.pending = Some(value);
            }
            _ => self.show(progress.call_id, value, at),
        }
    }

    /// 把 `value` 写进那一步、记下这一刻推过。
    fn show(&mut self, call: CallId, value: String, at: Timestamp) {
        let line = self.progress.calls.entry(call).or_default();
        line.shown = Some(value.clone());
        line.pending = None;
        line.pushed = Some(at);
        if let Some(id) = self.calls.get(&call).cloned() {
            self.touch(&id, |entry| {
                if let Body::Tool(tool) = &mut entry.body {
                    tool.progress = Some(value);
                }
            });
        }
    }

    /// 压着的进度最早什么时候到点；没有压着的没有。
    #[must_use]
    pub fn due(&self) -> Option<Timestamp> {
        self.progress.calls.values().filter_map(Line::due).min()
    }

    /// 到了 `now`：到点的压着的推出去，交回这几条变化。
    pub fn tick(&mut self, now: Timestamp) -> Vec<Change> {
        let ready: Vec<(CallId, String)> = self
            .progress
            .calls
            .iter()
            .filter(|(_, line)| line.due().is_some_and(|due| due <= now))
            .filter_map(|(call, line)| Some((*call, line.pending.clone()?)))
            .collect();
        for (call, value) in ready {
            self.show(call, value, now);
        }
        self.drain()
    }

    /// 调用 `call` 有了结果：它的进度都扔掉。
    pub(super) fn progress_over(&mut self, call: CallId) {
        self.progress.calls.remove(&call);
    }
}

#[cfg(test)]
mod tests;
