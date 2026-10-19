//! 记下的记忆：记忆日志里的事件怎么写、怎么读，照它算出的底账（`docs/blueprint/memory.md`「对外的样子」的记忆日志、底账，
//! 施工 R-3 上）。
//!
//! - 事件用内核的外壳，种类是 `ext.memory.*`：内核不认识，原样留着（`03-事件模型.md` 第三节）；这里照自己的格读写 `body`。
//! - 只追加：改一条是新写一条 `saved` 指着旧的（`replaces`），作废是另写一条 `retired`，原文一直在日志里（mem0 弃了原地
//!   改写，`docs/reviews/2026-10-07-记忆知识库embedding调研.md` 第二节）。
//! - 底账 [`MemoryBook`] 只是照日志算：改一条不存在的、作废一条不存在的，照样记下这条事件，不影响别的；日志是真相，底账
//!   不拒。

mod book;
#[cfg(test)]
mod tests;

pub use book::{Entry, MemoryBook};

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{EventKind, Seq, SessionId, TurnId, VenueId};
use miyu_kernel::origin::By;
use miyu_kernel::raw::RawJson;
use miyu_kernel::time::Timestamp;

/// 记下一条（写了 `replaces` 的是改一条）。
const SAVED: &str = "ext.memory.saved";
/// 作废一条。
const RETIRED: &str = "ext.memory.retired";
/// 清空（施工 R-3 补）。
const CLEARED: &str = "ext.memory.cleared";
/// 抽到哪（施工 R-6 上）。
const EXTRACTED: &str = "ext.memory.extracted";
/// 合并写的摘要（施工 R-7 上）。
const SUMMARY: &str = "ext.memory.summary";
/// 合到哪（施工 R-7 上）。
const MERGED: &str = "ext.memory.merged";
/// 恢复作废的（施工 R-12 下）。
const RESTORED: &str = "ext.memory.restored";
/// 换一条的关键词（施工 R-15 上）。
const TAGGED: &str = "ext.memory.tagged";
/// 开始合一批（施工 R-17）。
const MERGING: &str = "ext.memory.merging";
/// 这一批这一次没合成（施工 R-17）。
const MERGE_FAILED: &str = "ext.memory.merge_failed";
/// 整理顺带重看的旧记忆轮到了哪（施工 R-19）。
const SWEPT: &str = "ext.memory.swept";

/// 一条的关键词最多几个字（施工 R-15 上）：一行，逗号隔开，十来个词。
pub const KEYWORD_CHARS: usize = 200;

/// 整理一行关键词（施工 R-15 上）：去掉前后的空白；超过 [`KEYWORD_CHARS`] 字的在那以内最后一个逗号（`,`、`，`、`、`）处截断，
/// 一个逗号都没有的照字数截；空的没有。她经工具给的照它截，不拒（`remember`）。
#[must_use]
pub fn keywords_of(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let cut: String = raw.chars().take(KEYWORD_CHARS).collect();
    let cut = if cut.len() < raw.len() {
        match cut.rfind([',', '，', '、']) {
            Some(at) => cut[..at].to_string(),
            None => cut,
        }
    } else {
        cut
    };
    let cut = cut.trim().to_string();
    (!cut.is_empty()).then_some(cut)
}

/// 一条进检索库的字（施工 R-15 上，`memory.md` 第二条）：关键词索引、向量都照它，正文后面换行接上关键词；没有关键词的就是正文。
/// 给她看的地方（常驻那一块、联想、`memory_search`、整理的请求）照旧只用正文。
#[must_use]
pub fn indexed(text: &str, keywords: Option<&str>) -> String {
    match keywords {
        Some(keywords) if !keywords.is_empty() => format!("{text}\n{keywords}"),
        _ => text.to_string(),
    }
}

/// 出厂认识的四类（2026-10-07 项目主人定）：关于你、你要她怎样、经历、长期有效的事实。日志里别的类原样留着、照常列出；
/// 她经工具记的照这张名单查（R-3 中）。
pub const CLASSES: [&str; 4] = ["user", "feedback", "episode", "reference"];

/// 一条记忆的编号：它在记忆日志里的序号前面加 `m`，例如 `m42`。从序号算，不写进 `body`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MemoryId(Seq);

impl MemoryId {
    /// 记忆日志里第 `seq` 条记下的那一条。
    pub fn new(seq: Seq) -> MemoryId {
        MemoryId(seq)
    }

    /// 读 `m42` 这种写法；别的写法（没有 `m`、大写、0、不是数）是 `None`。
    pub fn parse(text: &str) -> Option<MemoryId> {
        let digits = text.strip_prefix('m')?;
        if !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        digits.parse().ok().and_then(Seq::new).map(MemoryId)
    }

    /// 它在记忆日志里的序号。
    pub fn seq(self) -> Seq {
        self.0
    }
}

impl fmt::Display for MemoryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "m{}", self.0.get())
    }
}

impl Serialize for MemoryId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for MemoryId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        MemoryId::parse(&text)
            .ok_or_else(|| serde::de::Error::custom(format!("not a memory id: {text}")))
    }
}

/// 一处出处：哪个会话的哪一轮。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    /// 会话编号。
    pub session: SessionId,
    /// 回合编号。
    pub turn: TurnId,
}

/// `ext.memory.saved` 的 `body`：记下一条，写了 `replaces` 的是改那一条。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Saved {
    /// 类：出厂四类之一（[`CLASSES`]），别的原样留着。
    pub class: String,
    /// 正文，一条不超过 120 字（工具查，R-3 中）。
    pub text: String,
    /// 出处，几处；人在界面上记的是空的。
    pub sources: Vec<Source>,
    /// 听众：出处那几轮会被谁看到，几个「谁」（`17-记忆.md` L6）。
    pub audience: Vec<By>,
    /// 改的是哪一条；没有的是新记的。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaces: Option<MemoryId>,
    /// 说的是哪天的事，写法自由；没有的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<String>,
    /// 在哪个场所说的（施工 R-13 上，`memory.md` 第九条）：场所里记的是那个场所，只在那个场所和主人那边冒出来；主人那边记的、
    /// 以前记的没有，不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub venue: Option<VenueId>,
    /// 只用来检索的关键词（施工 R-15 上）：一行，逗号隔开，最多 [`KEYWORD_CHARS`] 字；没有的不写（以前记的都没有）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keywords: Option<String>,
}

/// `ext.memory.tagged` 的 `body`：只换一条的关键词，编号不变（施工 R-15 上）。空的是去掉。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tagged {
    /// 哪一条。
    pub id: MemoryId,
    /// 新的关键词。
    pub keywords: String,
}

/// `ext.memory.retired` 的 `body`：作废一条。整理作废的另写哪一种、留下的是哪一条（施工 R-17）；人作废的、R-17 以前的不写。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Retired {
    /// 作废哪一条。
    pub id: MemoryId,
    /// 为什么。
    pub why: String,
    /// 哪一种：核心照规矩核对过的（`miyu_recall::merge`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<RetireKind>,
    /// 留下的那一条：取代它的（`outdated`）、和它重复的（`repeat`）；`worthless` 没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept: Option<MemoryId>,
}

/// 整理作废的是哪一种（施工 R-16 起模型交、核心核对，R-17 起记进日志）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RetireKind {
    /// 被后记的一条取代了。
    Outdated,
    /// 和另一条说的一样。
    Repeat,
    /// 以后用不着：一次性的请求、转眼就过的状态、测试、客套话。
    Worthless,
}

/// `ext.memory.restored` 的 `body`：作废了的那一条不算作废了（施工 R-12 下，`memory.md` 第二条第 6 款）：人恢复误删的。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Restored {
    /// 恢复哪一条。
    pub id: MemoryId,
}

/// `ext.memory.cleared` 的 `body`：人清空了（施工 R-3 补，`memory.md` 第二条第 5 款）。那以前的、合条件的都清掉，原文还在
/// 日志里。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cleared {
    /// 只清从这个会话来的（出处全在它里面的）；没有的是整间。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionId>,
}

/// `ext.memory.extracted` 的 `body`：一个会话抽到了哪（施工 R-6 上，`memory.md` 第六条）。记下这一段的候选以后才写：中途
/// 崩了的下次重抽这一段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extracted {
    /// 哪个会话。
    pub session: SessionId,
    /// 抽到会话日志的第几条：下次从它后面接着。
    pub upto: Seq,
    /// 记下了几条。
    pub count: u32,
    /// 整段跳过的为什么；没跳的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<Skipped>,
}

/// `ext.memory.summary` 的 `body`：合并写的一段摘要（施工 R-7 上，`memory.md` 第七条第 4 款）。不是一条记忆，不进记忆库；
/// 底账照最后一份。写下以后改了、作废了它合进去的一条，或者清空了，它就不算了（施工 R-7 下）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    /// 正文。
    pub text: String,
    /// 合到记忆日志的第几条：照它以前的写的。
    pub upto: Seq,
}

/// `ext.memory.merged` 的 `body`：一次合并做完了（施工 R-7 上，`memory.md` 第七条第 4、6 款）。改的、作废的、摘要都记下以后
/// 才写：中途崩了的下次重合这一批。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Merged {
    /// 合到记忆日志的第几条：交进去的新记的里最后那一条，下次从它后面接着。
    pub upto: Seq,
    /// 交进去几条（新记的加相关的）。
    pub given: u32,
    /// 改了几条。
    pub revised: u32,
    /// 作废几条。
    pub retired: u32,
    /// 这一批连着合不成三次，跳过了；没失败的不写。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub failed: bool,
}

/// `ext.memory.merging` 的 `body`：开始合一批（施工 R-17，`memory.md` 第七条第 10 款）。发请求以前写；合完的 `merged`、没合成
/// 的 `merge_failed` 带同一个 `upto`，照它对上。订阅着的头照它画进度：`left` 是总数，各批的 `fresh` 加起来是合了多少。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Merging {
    /// 合到记忆日志的第几条：这一批新记的里最后那一条。
    pub upto: Seq,
    /// 这一批交几条新记的（不算带上的相关旧记忆）。
    pub fresh: u32,
    /// 还有几条新记的要合：这一批的算在里面。
    pub left: u32,
    /// 人叫的（`/dream`、`memory.dream`）；后台自己合的是 `false`。
    pub manual: bool,
    /// 这一批交给模型的是哪几条（新记的、相关的旧记忆、顺带重看的，照编号；施工 R-21）：头等的时候让它们一条条浮过去。
    /// R-21 以前的没有。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub given: Vec<MemoryId>,
}

/// `ext.memory.merge_failed` 的 `body`：`upto` 那一批这一次没合成（施工 R-17）。连着第三次的后面另跟一条 `merged`（`failed`），
/// 跳过这一批；核心半路停了的，下次开这一间时补一条 `interrupted`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeFailed {
    /// 哪一批：和它的 `merging` 一样。
    pub upto: Seq,
    /// 英文的一句为什么，同运行日志；核心半路停了的是 `interrupted`。
    pub why: String,
    /// 连着第几次；半路停了的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tries: Option<u32>,
}

/// `ext.memory.swept` 的 `body`：一趟整理的头一批顺带重看了几条旧记忆、轮到第几条（施工 R-19，`memory.md` 第七条第 11 款）。
/// 这一批合成了才记，跟在它的 `merged` 后面；下一趟从 `upto` 后面接着，轮完一圈从头来。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Swept {
    /// 重看的里最后那一条（照编号）。
    pub upto: Seq,
    /// 重看了几条。
    pub count: u32,
}

/// 核心半路停了，`merging` 没有收尾（施工 R-17）。
pub const INTERRUPTED: &str = "interrupted";

/// 一段为什么整段跳过。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Skipped {
    /// 这一段里她调过 `remember`、`forget`：她当场记过了。
    Remembered,
    /// 同一段连着抽不成三次。
    Failed,
}

/// 记忆日志里的一条事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryEvent {
    /// 记下（或改）一条。
    Saved(Saved),
    /// 作废一条。
    Retired(Retired),
    /// 清空（施工 R-3 补）。
    Cleared(Cleared),
    /// 一个会话抽到了哪（施工 R-6 上）。
    Extracted(Extracted),
    /// 合并写的摘要（施工 R-7 上）。
    Summary(Summary),
    /// 一次合并做完了（施工 R-7 上）。
    Merged(Merged),
    /// 恢复作废的（施工 R-12 下）。
    Restored(Restored),
    /// 换一条的关键词（施工 R-15 上）。
    Tagged(Tagged),
    /// 开始合一批（施工 R-17）。
    Merging(Merging),
    /// 这一批这一次没合成（施工 R-17）。
    MergeFailed(MergeFailed),
    /// 整理顺带重看的旧记忆轮到了哪（施工 R-19）。
    Swept(Swept),
}

/// 写成记忆日志里的一条事件：第 `seq` 条，时刻 `at`，`by` 写它的那一方。
///
/// # Errors
///
/// `body` 写不成 JSON（不会发生：格都是字符串、数和列表），如实交回。
pub fn to_event(seq: Seq, at: Timestamp, by: By, event: &MemoryEvent) -> Result<Event, String> {
    let (kind, json) = match event {
        MemoryEvent::Saved(saved) => (SAVED, serde_json::to_string(saved)),
        MemoryEvent::Retired(retired) => (RETIRED, serde_json::to_string(retired)),
        MemoryEvent::Cleared(cleared) => (CLEARED, serde_json::to_string(cleared)),
        MemoryEvent::Extracted(extracted) => (EXTRACTED, serde_json::to_string(extracted)),
        MemoryEvent::Summary(summary) => (SUMMARY, serde_json::to_string(summary)),
        MemoryEvent::Merged(merged) => (MERGED, serde_json::to_string(merged)),
        MemoryEvent::Restored(restored) => (RESTORED, serde_json::to_string(restored)),
        MemoryEvent::Tagged(tagged) => (TAGGED, serde_json::to_string(tagged)),
        MemoryEvent::Merging(merging) => (MERGING, serde_json::to_string(merging)),
        MemoryEvent::MergeFailed(failed) => (MERGE_FAILED, serde_json::to_string(failed)),
        MemoryEvent::Swept(swept) => (SWEPT, serde_json::to_string(swept)),
    };
    let json = json.map_err(|error| error.to_string())?;
    let body: RawJson = serde_json::from_str(&json).map_err(|error| error.to_string())?;
    let kind = EventKind::parse(kind).map_err(|error| error.to_string())?;
    Ok(Event {
        seq,
        at,
        turn: None,
        by,
        cause: None,
        body: Body::Unknown { kind, body },
    })
}

/// 读一条事件：不是记忆的种类交回 `None`；是记忆的、`body` 读不懂的交回为什么。
pub fn from_event(event: &Event) -> Option<Result<MemoryEvent, String>> {
    let Body::Unknown { kind, body } = &event.body else {
        return None;
    };
    let read = match kind.as_str() {
        SAVED => serde_json::from_str(body.get()).map(MemoryEvent::Saved),
        RETIRED => serde_json::from_str(body.get()).map(MemoryEvent::Retired),
        CLEARED => serde_json::from_str(body.get()).map(MemoryEvent::Cleared),
        EXTRACTED => serde_json::from_str(body.get()).map(MemoryEvent::Extracted),
        SUMMARY => serde_json::from_str(body.get()).map(MemoryEvent::Summary),
        MERGED => serde_json::from_str(body.get()).map(MemoryEvent::Merged),
        RESTORED => serde_json::from_str(body.get()).map(MemoryEvent::Restored),
        TAGGED => serde_json::from_str(body.get()).map(MemoryEvent::Tagged),
        MERGING => serde_json::from_str(body.get()).map(MemoryEvent::Merging),
        MERGE_FAILED => serde_json::from_str(body.get()).map(MemoryEvent::MergeFailed),
        SWEPT => serde_json::from_str(body.get()).map(MemoryEvent::Swept),
        _ => return None,
    };
    Some(read.map_err(|error| format!("body of {kind} not readable: {error}")))
}
