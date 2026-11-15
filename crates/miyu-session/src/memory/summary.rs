//! 常驻的记忆摘要（施工 R-4 上，`docs/blueprint/memory.md` 第三条）：会话的第一轮、压缩以后的第一轮，回合开始的挂接点交回
//! 一块事实，在触发的那句前面，以后原样回放（缓存只在第一轮多写一次，第十条）。
//!
//! - 交不交：内核交来的 `present` 里没有这个模块、这一类的一块才交（第一轮、压缩以后、撤掉带着它的那一轮以后）；人格记忆这时
//!   没装的不交（施工 R-10）。
//! - 交什么：这一间里现在算数的、出处活着的、听众合的，新的在前，一条一行，和 `memory_search` 的一行一个写法；`refs` 是这几条
//!   的编号。合并写过、还算数的摘要的（施工 R-7 下，第七条第 8 款），头一行是摘要，后面只接上次合并以后新记的（不算合并自己
//!   改出来的）。排名（半衰期、用到几次）随 R-4 下。
//! - 上限：外壳加几行不超过 [`LIMIT`] 字节，截在一条的边界上，最后一行说还有几条。
//! - 她记下的自己的变化（`change`，施工 LF-11，第三条第 10 款）：现在算数的不管摘要合到哪都列，在开头那一行后面、摘要那一行
//!   前面，先一行标题，下面一条一行、旧的在前；不占 [`LIMIT`]、不算进截了的条数，不在摘要以后新记的那一段里再列。一条都没有
//!   的，这一块和没有这一类时逐字节一样。

use std::path::Path;

use miyu_kernel::event::ContextInjected;
use miyu_kernel::facts::Present;
use miyu_kernel::id::{FactKind, ModuleId};
use miyu_kernel::session::Injection;
use miyu_kernel::template::Template;
use miyu_kernel::time::UtcOffset;
use miyu_recall::{CHANGE, Entry};
use miyu_tool::load::{self, LoadError};
use miyu_tool::{Line, MemoryLines};

use super::merge::made_by_merge;
use super::{Filter, Keeper};

/// 记忆这个软件包在资源目录里的名字（和 `miyu-memory` 的同一个；会话这一层不依赖软件包）。
const SOFTWARE: &str = "memory";

/// 注入那一块的模块和类别：都叫 `memory`。
const NAME: &str = "memory";

/// 一块最多几个字节（`memory.md` 第三条第 3 款）：起点是约 1000 token。中文为主的一块照字节/4 折算会少算，2026-10-08 在
/// 开发端点量过：4000 字节 1298 token，3000 字节 964 token，所以取 3000。
pub const LIMIT: usize = 3000;

/// 摘要那一块给模型看的字：外壳、一行（和 `memory_search` 的同一份）、截了的那一句、合并写的摘要那一行（施工 R-7 下）
/// （`resources/software/memory/`，登记簿）。
#[derive(Debug, Clone)]
pub struct SummaryTexts {
    open: Template,
    lines: MemoryLines,
    more: Template,
    close: Template,
    digest: Template,
    /// 她记下的自己的变化那几行上面的标题（施工 LF-11）：`summary/changes.txt`。
    changes: Template,
    /// 认出来那一块（施工 E-1 补，`known.rs`）：`card/person.txt`（`{who}`、`{text}`）。
    pub(super) card: Template,
}

impl SummaryTexts {
    /// 从资源目录 `resources` 读：`summary/open.txt`、`summary/close.txt`、`summary/more.txt`（`{count}`）、`summary/digest.txt`
    /// （`{text}`），她的变化那几行的标题 `summary/changes.txt`（施工 LF-11），认出来那一块 `card/person.txt`（`{who}`、`{text}`，施工 E-1 补），一行照 `memory_search/memory.txt`
    /// （`{id}`、`{class}`、`{date}`、`{text}`）。
    ///
    /// # Errors
    ///
    /// 读不出来；不是合写法的模板，或者要了别的字段。
    pub fn load(resources: &Path) -> Result<SummaryTexts, LoadError> {
        Ok(SummaryTexts {
            open: load::text(resources, SOFTWARE, "summary", "open", &[])?,
            lines: MemoryLines::load(resources, SOFTWARE)?,
            more: load::text(resources, SOFTWARE, "summary", "more", &["count"])?,
            close: load::text(resources, SOFTWARE, "summary", "close", &[])?,
            digest: load::text(resources, SOFTWARE, "summary", "digest", &["text"])?,
            changes: load::text(resources, SOFTWARE, "summary", "changes", &[])?,
            card: load::text(resources, SOFTWARE, "card", "person", &["who", "text"])?,
        })
    }
}

impl Keeper {
    /// 这一轮要不要交摘要、交什么（这一段上下文里已经有的、一条都没有的、读不了的、核心没读到外壳的字的不交）。`offset`
    /// 是会话的时区。碰磁盘，调的一方放在阻塞线程里。
    pub(crate) fn summary(&self, offset: UtcOffset, present: &[Present]) -> Option<Injection> {
        if !self.installed() {
            return None;
        }
        let name =
            |present: &Present| present.module.as_str() == NAME && present.kind.as_str() == NAME;
        let texts = self.memory.summary.as_ref()?;
        if present.iter().any(name) {
            return None;
        }
        let filter = Filter {
            class: None,
            from: None,
            forgotten: false,
            limit: usize::MAX,
        };
        // 摘要那一行是照全部记忆写的一份（第七条），哪儿都交（施工 R-29：原来只在主人那边交）。
        let read = self.list(&filter).and_then(|entries| {
            let digest = self
                .log()
                .map_err(|refused| format!("{refused:?}"))?
                .book(|book| book.summary().map(|(text, upto)| (text.to_string(), upto)));
            Ok((entries, digest))
        });
        let (entries, digest) = match read {
            Ok(read) => read,
            Err(error) => {
                tracing::warn!(target: crate::TARGET, error = error.as_str(), "memory summary not read");
                return None;
            }
        };
        // 她的变化单列（施工 LF-11）：旧的在前。
        let (mut changes, mut entries): (Vec<Entry>, Vec<Entry>) =
            entries.into_iter().partition(|entry| entry.class == CHANGE);
        changes.reverse();
        // 有摘要的只接它以后新记的：合进去的、合并自己改出来的都在摘要里了。
        if let Some((_, upto)) = &digest {
            entries.retain(|entry| entry.id.seq() > *upto && !made_by_merge(entry));
        }
        if entries.is_empty() && digest.is_none() && changes.is_empty() {
            return None;
        }
        let digest = digest.as_ref().map(|(text, _)| text.as_str());
        let changes = (!changes.is_empty()).then_some(changes.as_slice());
        let (text, refs) = listed(texts, (changes, digest), &entries, offset);
        Some(Injection {
            module: ModuleId::parse(NAME).expect("合写法"),
            fact: ContextInjected {
                kind: FactKind::parse(NAME).expect("合写法"),
                text,
                refs,
                after: false,
            },
        })
    }
}

/// 拼常驻那一块的原文和 `refs`（人生会话醒来时交新记的那一块也照它拼，施工 LF-6，`fresh.rs`，那边不带她的变化）：外壳里
/// 头几行是她的变化 `changes`（有的话，施工 LF-11：标题一行、一条一行、照给的先后，不占上限），接着是摘要 `digest`（有的话），
/// 后面一条一行、照 `entries` 的先后，除她的变化以外整块不超过 [`LIMIT`] 字节，超了截在一条的边界上、最后一行说还有几条
/// （只数 `entries`）。`refs` 是列出来的那几条（她的变化在前），摘要那一行不算。`offset` 是会话的时区。
pub(super) fn listed(
    texts: &SummaryTexts,
    (changes, digest): (Option<&[Entry]>, Option<&str>),
    entries: &[Entry],
    offset: UtcOffset,
) -> (String, Vec<String>) {
    let say = |template: &Template, fields: &[(&str, &str)]| {
        load::say(template, fields).trim_end().to_string()
    };
    let (open, close) = (say(&texts.open, &[]), say(&texts.close, &[]));
    let mut top = Vec::new();
    let mut refs = Vec::new();
    if let Some(changes) = changes {
        top.push(say(&texts.changes, &[]));
        for entry in changes {
            top.push(row(texts, entry, offset));
            refs.push(entry.id.to_string());
        }
    }
    let head = digest.map(|text| say(&texts.digest, &[("text", text)]));
    // 截了的那一句照最长的数留出地方：一条都放不下也说得出还有几条。
    let more = say(&texts.more, &[("count", &entries.len().to_string())]);
    let room = LIMIT.saturating_sub(
        open.len() + close.len() + more.len() + 3 + head.as_ref().map_or(0, |head| head.len() + 1),
    );
    let mut lines: Vec<String> = head.into_iter().collect();
    let header = lines.len();
    let mut used = 0;
    for entry in entries {
        let row = row(texts, entry, offset);
        if used + row.len() + 1 > room {
            break;
        }
        used += row.len() + 1;
        lines.push(row);
        refs.push(entry.id.to_string());
    }
    let left = entries.len() - (lines.len() - header);
    if left > 0 {
        lines.push(say(&texts.more, &[("count", &left.to_string())]));
    }
    top.extend(lines);
    (format!("{open}\n{}\n{close}\n", top.join("\n")), refs)
}

/// 一条一行，和 `memory_search` 的一行一个写法：日期照会话的时区 `offset`，正文里的换行换成空格。
fn row(texts: &SummaryTexts, entry: &Entry, offset: UtcOffset) -> String {
    let (id, date) = (entry.id.to_string(), entry.at.local_date(offset));
    let text = entry.text.replace('\n', " ");
    let class = miyu_recall::shown_class(&entry.class, entry.who.as_deref());
    texts
        .lines
        .say(&Line {
            id: &id,
            class: &class,
            date: &date,
            text: &text,
            venue: entry.venue.as_ref(),
            place: entry.place.as_deref(),
            retired: false,
        })
        .trim_end()
        .to_string()
}
