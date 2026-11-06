//! 缓存打点放在哪几块（`docs/blueprint/drivers/anthropic.md`「缓存打点」）：前缀即契约，打点只告诉这一家「到这里存一份」。
//!
//! 照这个先后算，同一块只打一次，最多 4 处：
//!
//! 1. 工具和 system 的末尾：system 那一块；system 是空的打在最后一件工具上；两样都没有的不打。
//! 2. 稳定区的末尾：前 `stable` 条消息（示范对话）写出的最后一块。
//! 3. 上一次请求的末尾：稳定区以后最后一条 assistant 前面那一条线上消息的最后一块；没有这样的 assistant 的不打。这一家
//!    每个打点最多往回找 20 块，这一次新加的块多了，第 4 处找不到上一次存的，第 3 处正好落在那里。
//! 4. 这一次请求的末尾：最后一条线上消息的最后一块。接了 respond 带的那一句的（施工 RT-1，`crate::tail`），打在它前面那一块上：
//!    那一句下一步就没有了，存到它前面的那一份下一步第 3 处正好接上。
//!
//! 打在一条消息里，落在指定的那一块或者它前面最后一个能打的块上：思考块不能打，往前找；整条都没有能打的，这一处不打。

use std::collections::BTreeSet;

use miyu_kernel::request::Request;

use super::messages::Written;

/// 打在哪几处。
#[derive(Debug, Default)]
pub(super) struct Marks {
    /// system 那一块打了。
    pub(super) system: bool,
    /// 打在第几件工具上。
    pub(super) tool: Option<usize>,
    /// 打在第几条线上的消息的第几块上：几处落在同一块上的只打一次。
    pub(super) messages: BTreeSet<(usize, usize)>,
}

/// 打点要看的写好的消息（施工 V-42 起两样都有：整份编的 [`Written`]，留着的历史现排的那一份）。
pub(super) trait Shape {
    /// 几条线上的消息。
    fn len(&self) -> usize;
    /// 第 `m` 条的角色。
    fn role(&self, m: usize) -> &'static str;
    /// 第 `m` 条是从统一的请求里第几条消息开始的。
    fn first(&self, m: usize) -> usize;
    /// 第 `m` 条有几块。
    fn blocks(&self, m: usize) -> usize;
    /// 第 `m` 条的第 `b` 块能不能打点（[`super::wire::Part::markable`]）。
    fn markable(&self, m: usize, b: usize) -> bool;
    /// 前 `stable` 条消息写出的最后一块（[`Written::stable_end`]）。
    fn stable_end(&self) -> Option<(usize, usize)>;
}

impl Shape for Written {
    fn len(&self) -> usize {
        self.messages.len()
    }

    fn role(&self, m: usize) -> &'static str {
        self.messages[m].role
    }

    fn first(&self, m: usize) -> usize {
        self.firsts[m]
    }

    fn blocks(&self, m: usize) -> usize {
        self.messages[m].content.len()
    }

    fn markable(&self, m: usize, b: usize) -> bool {
        self.messages[m].content[b].markable()
    }

    fn stable_end(&self) -> Option<(usize, usize)> {
        self.stable_end
    }
}

/// 照请求、工具面有几件、写好的消息算打点；`tail` 是最后一块是接上的那一句（施工 RT-1）。
pub(super) fn place(
    request: &Request,
    tools: Option<usize>,
    written: &impl Shape,
    tail: bool,
) -> Marks {
    let mut marks = Marks::default();
    if !request.system.is_empty() {
        marks.system = true;
    } else if let Some(last) = tools.and_then(|tools| tools.checked_sub(1)) {
        marks.tool = Some(last);
    }
    let last_block = |m: usize| (m, written.blocks(m).saturating_sub(1));
    let previous = (0..written.len())
        .rposition(|m| written.role(m) == "assistant" && written.first(m) >= request.stable)
        .and_then(|assistant| assistant.checked_sub(1))
        .map(last_block);
    let latest = written.len().checked_sub(1).and_then(|m| match tail {
        false => Some(last_block(m)),
        // 那一句前面那一块：同一条里还有别的块的就在这一条，那一句单独成了一条的落到上一条的末尾。
        true if written.blocks(m) > 1 => Some((m, written.blocks(m) - 2)),
        true => m.checked_sub(1).map(last_block),
    });
    for (m, upto) in [written.stable_end(), previous, latest]
        .into_iter()
        .flatten()
    {
        let found = (0..upto + 1).rposition(|b| written.markable(m, b));
        if let Some(b) = found {
            marks.messages.insert((m, b));
        }
    }
    marks
}
