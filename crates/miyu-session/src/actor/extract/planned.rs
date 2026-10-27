//! 照读回来的这一段定怎么抽（施工 PERF-4 从 actor 上挪出来）：拼一份留着一切的历史，照会话的组装器渲染 `after` 以后的那几段，
//! 认出记过的、谁说的，定怎么办。纯的、不碰 actor：一直在忙的会话的抽取（施工 E-5）在阻塞线程里调它，不占会话（大会话上
//! 一万多条事件拼一遍要几毫秒，原来在 actor 上同步做，下一轮的回合开头排在后面）；现在就整理那一路照旧在 actor 上调。

use miyu_kernel::assemble::Assembler;
use miyu_kernel::event::Event;
use miyu_kernel::history::History;
use miyu_kernel::id::Seq;
use miyu_kernel::time::UtcOffset;

use crate::memory::{ExtractTexts, Voices, plan, remembered_since};

/// 定怎么抽要的几样（都在 actor 上拿好，交进来的是这一刻的）。
pub(crate) struct Planning<'a> {
    /// 这个会话的组装器（`Session::assembler`）。
    pub(crate) assembler: &'a dyn Assembler,
    /// 群聊近况每一轮带多少字节、最多几条（[`super::Actor`] 照人格快照算的）；不是群会话的没有。
    pub(crate) budget: Option<usize>,
    pub(crate) limit: Option<usize>,
    /// 至少几轮才抽。
    pub(crate) min_turns: usize,
    pub(crate) offset: UtcOffset,
    pub(crate) texts: &'a ExtractTexts,
    /// 照称呼写主人（施工 R-6 再修）：没有的不写。
    pub(crate) name: Option<&'a str>,
}

/// `events` 里第 `after` 条以后的那一段怎么抽。
pub(crate) fn planned(events: Vec<Event>, after: Seq, how: &Planning<'_>) -> crate::memory::Plan {
    let mut history = History::whole();
    for event in events {
        history.append(event);
    }
    let spoken = how.assembler.spoken(&history, after);
    let remembered = remembered_since(&history, after);
    let mut voices = Voices::of(history.events(), how.budget);
    voices.limit = how.limit;
    plan(
        &spoken,
        &remembered,
        how.min_turns,
        how.offset,
        how.texts,
        &voices,
        how.name,
    )
}

