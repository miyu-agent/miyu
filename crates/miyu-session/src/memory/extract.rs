//! 抽取（施工 R-6 上，`docs/blueprint/memory.md` 第六条）：会话闲下来以后，把上次抽到以后的这一段里值得记的抽出来，记成她的
//! 记忆。这里是会话那一层的几样：给模型看的字、一个会话的抽取状态、照这一段拼请求、派出去的那一件活。什么时候起、这一段
//! 从哪来在 `actor/extract.rs`。
//!
//! - **这一段**：会话那一层照日志拼的历史（留着一切的那一份），交给会话的组装器渲染（`Session::spoken_in`），所以压缩过的也
//!   在。人的话接在它触发的那一轮上；还没答的那一句不算进来，等下次。每一句标出谁说的，群里每一轮只带她当时看到的那几句
//!   （施工 R-13 上，`extract/voices.rs`）。
//! - **拼法**（单发）：指令在前，后面是这一段的几轮，一轮一块（编号、日期、人的话、她的回答），一条 user，不带 system、
//!   工具面。最多 [`ROOM`] 字节：放不下的照先后取最老的几轮，`upto` 记到取到的最后一轮；一轮自己就放不下的留头尾、截中间，
//!   照样抽，`upto` 照样往前走，不卡在一轮上。
//! - **忘掉的**：这一段开始以后作废的、改掉的，接在这一段后面列出来，说清别再记（施工 R-6 修）；没有的请求一字不变。
//! - **跳过**：这一段里她调过 `remember`、`forget` 的那几轮不交（她当场记过了），别的轮照抽；整段都是这种的只记抽到了哪
//!   （施工 E-5：原来整段跳过，群里一段几十轮，她记一句就扔掉整段）。
//! - **发**：经一次性入口发 `memory.organizer`（没写的照 `models.chat`），用途 `memory`，记在会话属主的账上。发出去以前遮掉
//!   key（配置里引用的密钥的原文、常见的写法），交回来的再遮一遍。
//! - **出错**：这一段不往前挪，下次闲了再抽；同一段连着三次不成的跳过，记 `WARN memory extraction failed`。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use miyu_config::Value;
use miyu_config::secret::Reference;
use miyu_kernel::assemble::Spoken;
use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::history::History;
use miyu_kernel::id::{Seq, TurnId};
use miyu_kernel::template::{Template, escape};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_recall::redact::KeyShapes;
use miyu_store::blob::Blobs;
use miyu_tool::load::{self, LoadError, say};
use miyu_tool::{FORGET, MemoryLines, REMEMBER};

use crate::config::Turn;
use crate::route::OneShot;

/// 这一段最多几个字节：和回顾的对话记录一个量级（约 8000 token）。
pub(crate) const ROOM: usize = 32 * 1024;

/// 一轮和一轮之间。
const GAP: &str = "\n\n";

/// 用途：记运行日志、记账，key 照它钉。
const PURPOSE: &str = "memory";

/// 同一段连着抽不成几次就跳过。
const TRIES: u32 = 3;

/// 记忆这个软件包在资源目录里的名字（和 `summary.rs` 的同一个）。
const PACKAGE: &str = "memory";

/// 这一段以后忘掉的最多列几条（施工 R-6 修）：一个闲置窗口里忘掉的一般只有几条，合并一次作废的多，新的在前够用。
pub(crate) const DROPPED: usize = 20;

/// 抽取给模型看的字（`resources/software/memory/extract/`，登记簿）：指令、一轮的头尾、人的话、她的回答、截了中间的那一句、
/// 这一段以后忘掉的那一句（一条一行照 `memory_search/memory.txt`），没有称呼时主人写的那一句（`common/owner-unnamed.txt`，
/// 施工 R-6 再修）。换进去的字照模板的规矩转义成一行
/// （`miyu_kernel::template`）：人的话伪造不了一轮的尾巴。
#[derive(Debug, Clone)]
pub struct ExtractTexts {
    instruction: Template,
    open: Template,
    close: Template,
    owner: Template,
    person: Template,
    assistant: Template,
    excerpted: Template,
    pub(super) dropped: Template,
    pub(super) lines: MemoryLines,
    unnamed: Template,
}

impl ExtractTexts {
    /// 从资源目录 `resources` 读：`extract/instruction.txt`（`{owner}`）、`turn-open.txt`（`{turn}`、`{date}`）、`turn-close.txt`、
    /// `owner.txt`、`assistant.txt`（`{text}`）、`person.txt`（`{name}`、`{text}`）、`excerpted.txt`、`dropped.txt`，一条忘掉的
    /// 照 `memory_search/memory.txt`（`{id}`、`{class}`、`{date}`、`{text}`），`common/owner-unnamed.txt`。
    ///
    /// # Errors
    ///
    /// 哪一份读不出来、写法不对、要了别的字段。
    pub fn load(resources: &Path) -> Result<ExtractTexts, LoadError> {
        let text =
            |name: &str, fields: &[&str]| load::text(resources, PACKAGE, "extract", name, fields);
        Ok(ExtractTexts {
            instruction: text("instruction", &["owner"])?,
            open: text("turn-open", &["turn", "date"])?,
            close: text("turn-close", &[])?,
            owner: text("owner", &["text"])?,
            person: text("person", &["name", "text"])?,
            assistant: text("assistant", &["text"])?,
            excerpted: text("excerpted", &[])?,
            dropped: text("dropped", &[])?,
            lines: MemoryLines::load(resources, PACKAGE)?,
            unnamed: load::text(resources, PACKAGE, "common", "owner-unnamed", &[])?,
        })
    }
}

/// 抽取要的几样：核心起来时交给记忆一次（[`super::Memory::give_extraction`]）；没交的不抽。
#[derive(Clone)]
pub struct Extraction {
    /// 给模型看的字。
    pub texts: ExtractTexts,
    /// 常见的 key 写法（`resources/core/memory/secrets.toml`）。
    pub shapes: KeyShapes,
    /// 一次性入口：照 `memory.organizer` 发。
    pub ask: OneShot,
    /// 编码要的 blob：抽取的请求只有字，用不上，一次性入口要它。
    pub blobs: Blobs,
    /// 闲多久才抽：没有的照配置 `memory.extract_idle`；测试里设短的，不用真等一分钟。
    pub idle: Option<std::time::Duration>,
    /// 一直不闲的会话最早没抽的那一轮结束了多久就不等闲（施工 E-5）：没有的照配置 `memory.extract_max_wait`；测试里设短的。
    pub max_wait: Option<std::time::Duration>,
    /// 合并给模型看的字（施工 R-7 上）：合并在抽取记下以后起，照同一个一次性入口发。
    pub merge: super::MergeTexts,
    /// 写关键词给模型看的字（施工 R-15 下）：抽取记下以后、合并落完以后另发一次，照同一个一次性入口。没交的不写（核心总交；
    /// 测试场地照要不要写关键词给）。
    pub tag: Option<super::TagTexts>,
}

impl std::fmt::Debug for Extraction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Extraction")
            .field("shapes", &self.shapes)
            .finish_non_exhaustive()
    }
}

/// 一轮：编号、她回答的时刻、人这边的几句（谁说的、字）、她的回答、她那一条的序号。
struct Exchange {
    turn: TurnId,
    at: Timestamp,
    said: Vec<(Voice, String)>,
    answer: String,
    seq: Seq,
}

/// 把几段话接成几轮：人的话接在它后面那一轮她的回答上；最后还没答的那几句不算；不是人说的不进（`voices`）。群会话的每一轮
/// 只留离它最近、装得进预算的那几句，一句都装不下的也留最后那一句。
fn exchanges(spoken: &[Spoken], voices: &Voices) -> Vec<Exchange> {
    let mut said = Vec::new();
    let mut exchanges = Vec::new();
    for one in spoken {
        match (one.assistant, one.turn) {
            (true, Some(turn)) => {
                let mut said = std::mem::take(&mut said);
                if let Some(budget) = voices.budget {
                    said = within(said, budget, voices.limit);
                }
                exchanges.push(Exchange {
                    turn,
                    at: one.at,
                    said,
                    answer: one.text.clone(),
                    seq: one.seq,
                });
            }
            (true, None) => {}
            (false, _) => {
                if let Some(voice) = voices.said.get(&one.seq) {
                    said.push((voice.clone(), one.text.clone()));
                }
            }
        }
    }
    exchanges
}

/// 从最新往前装，装不下就停（同群聊近况，`miyu-assemble` 的 `group/recent.rs`）；写了 `limit` 的最多装这么多句；至少留最后
/// 那一句。
fn within(said: Vec<(Voice, String)>, budget: usize, limit: Option<usize>) -> Vec<(Voice, String)> {
    let mut used = 0;
    let keep = said
        .iter()
        .rev()
        .take_while(|(_, text)| {
            used += text.len() + 1;
            used <= budget
        })
        .count()
        .min(limit.unwrap_or(usize::MAX))
        .max(1);
    let skip = said.len().saturating_sub(keep);
    said.into_iter().skip(skip).collect()
}

/// 这一段怎么办。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Plan {
    /// 答了的轮数不够：等下次。
    Wait,
    /// 整段跳过（每一轮她都当场记过了），只记抽到第 `upto` 条。
    Skip {
        /// 抽到第几条。
        upto: Seq,
    },
    /// 发出去抽。
    Ask {
        /// 抽到第几条：取到的最后一轮她那一条。
        upto: Seq,
        /// 请求的字：指令接这一段。
        text: String,
        /// 这一段里的几轮的编号：交回的候选只认这几轮。
        turns: BTreeSet<u64>,
        /// 放不下、还剩几轮没取：抽完接着抽剩下的。
        more: bool,
        /// 这一段第一句话的时刻：这以后忘掉的接在后面（施工 R-6 修）。
        since: Timestamp,
        /// 这一段里说话的人怎么叫（施工 E-1 上，[`Voices::names`]）连同主人的称呼：抽出来的认识的人照它换成平台编号。
        names: BTreeMap<String, Option<String>>,
        /// 这一段里她在场所里被叫过的名字（施工 E-1 修三，[`Voices::me`]）：和这一间人格的名字一起算她。
        me: BTreeSet<String>,
        /// 交进去的每一轮那时的群名（施工 E-4 上，[`Voices::places`]）：轮的编号对着群名，没有的不在里面。记下的照它写在哪个群。
        places: BTreeMap<u64, String>,
    },
}

/// 照这一段 `spoken`（她调过 `remember`、`forget` 的那几轮 `remembered`，每一句是谁说的 `voices`）定怎么办：答了的轮数至少
/// `min_turns`；日期照会话的时区 `offset`；主人照称呼 `name` 写，没有的写「用户」那一句（施工 R-6 再修）。
pub(crate) fn plan(
    spoken: &[Spoken],
    remembered: &BTreeSet<TurnId>,
    min_turns: usize,
    offset: UtcOffset,
    texts: &ExtractTexts,
    voices: &Voices,
    name: Option<&str>,
) -> Plan {
    let exchanges = exchanges(spoken, voices);
    let (Some(last), Some(first)) = (
        exchanges.last().map(|exchange| exchange.seq),
        spoken.first(),
    ) else {
        return Plan::Wait;
    };
    if exchanges.len() < min_turns.max(1) {
        return Plan::Wait;
    }
    // 她当场记过的那几轮不交（施工 E-5）：整段都是的只记抽到了哪。
    if exchanges
        .iter()
        .all(|exchange| remembered.contains(&exchange.turn))
    {
        return Plan::Skip { upto: last };
    }
    let mut blocks: Vec<String> = Vec::new();
    let mut turns = BTreeSet::new();
    let mut places = BTreeMap::new();
    let mut upto = last;
    let mut bytes = 0;
    for exchange in &exchanges {
        if remembered.contains(&exchange.turn) {
            upto = exchange.seq;
            continue;
        }
        let block = render(exchange, offset, texts);
        let needed = block.len() + if blocks.is_empty() { 0 } else { GAP.len() };
        if !blocks.is_empty() && bytes + needed > ROOM {
            break;
        }
        // 一轮自己就放不下的截中间也抽：不卡在一轮上。
        let block = match block.len() > ROOM {
            true => excerpt(&block, ROOM, texts),
            false => block,
        };
        bytes += needed;
        blocks.push(block);
        turns.insert(exchange.turn.started().get());
        if let Some(place) = super::keeper::place_at(&voices.places, exchange.seq) {
            places.insert(exchange.turn.started().get(), place);
        }
        upto = exchange.seq;
        if bytes >= ROOM {
            break;
        }
    }
    let owner = super::owner::owner(name, &texts.unnamed);
    let instruction = say(&texts.instruction, &[("owner", &owner)]);
    let mut names = voices.names.clone();
    names.insert(owner, None);
    Plan::Ask {
        upto,
        text: format!("{}\n{}", instruction.trim_end(), blocks.join(GAP)),
        turns,
        more: upto != last,
        since: first.at,
        names,
        me: voices.me.clone(),
        places,
    }
}

/// 上次抽到第 `after` 条以后她调过 `remember`、`forget` 的那几轮（施工 E-5）：这几轮她当场记过了，抽取不交。口径同
/// [`History::called_since`]：撤掉的回合不算。
pub(crate) fn remembered_since(history: &History, after: Seq) -> BTreeSet<TurnId> {
    history
        .ordered()
        .into_iter()
        .filter(|event| event.seq > after)
        .filter_map(|event| match &event.body {
            Body::MessageAssistant(reply)
                if reply.blocks.iter().any(|block| {
                    matches!(block, Block::ToolCall(call) if call.name == REMEMBER || call.name == FORGET)
                }) =>
            {
                event.turn
            }
            _ => None,
        })
        .collect()
}

/// 不等闲的那一次只取到读的那一刻结束了的最后一轮（施工 E-5）：最后一条 `turn.ended` 以后的不要，正在跑的那一轮、还没
/// 开始的人话留给下次（它们在抽到了哪以后）。一轮都没结束的什么都不留。
pub(crate) fn ended_only(events: &mut Vec<Event>) {
    let cut = events
        .iter()
        .rposition(|event| matches!(event.body, Body::TurnEnded(_)))
        .map_or(0, |at| at + 1);
    events.truncate(cut);
}

/// 一轮写成一块：头、人的话、她的回答、尾，一样一行。
fn render(exchange: &Exchange, offset: UtcOffset, texts: &ExtractTexts) -> String {
    let mut lines = vec![say(
        &texts.open,
        &[
            ("turn", &exchange.turn.started().get().to_string()),
            ("date", &exchange.at.local_date(offset)),
        ],
    )];
    lines.extend(exchange.said.iter().map(|(voice, text)| match voice {
        Voice::Owner => say(&texts.owner, &[("text", text)]),
        Voice::Named(name) => say(&texts.person, &[("name", name), ("text", text)]),
        // 群里那一行本来就带着名字和记号：只照模板的规矩转义成一行，伪造不了一轮的尾巴。
        Voice::Line => escape(text),
    }));
    lines.push(say(&texts.assistant, &[("text", &exchange.answer)]));
    lines.push(say(&texts.close, &[]));
    lines.join("\n")
}

/// 放不下的一块留头尾、截中间，至多 `room` 字节左右；截在字的边界上。
fn excerpt(block: &str, room: usize, texts: &ExtractTexts) -> String {
    let marker = say(&texts.excerpted, &[]);
    let half = room.saturating_sub(marker.len()) / 2;
    let head_end = (0..=half)
        .rev()
        .find(|at| block.is_char_boundary(*at))
        .unwrap_or(0);
    let tail_start = (block.len().saturating_sub(half)..=block.len())
        .find(|at| block.is_char_boundary(*at))
        .unwrap_or(block.len());
    format!("{}{marker}{}", &block[..head_end], &block[tail_start..])
}

/// 配置里引用的密钥的原文（照这一份配置取得到的）：发出去以前、记下以前遮掉。
pub(super) fn secrets(config: &Turn) -> Vec<String> {
    let values = config.resolved.values();
    let mut found = Vec::new();
    for key in values.keys() {
        let references: Vec<&Reference> = match values.get(key) {
            Some(Value::Secret(reference)) => vec![reference],
            Some(Value::List(list)) => list
                .iter()
                .filter_map(|value| match value {
                    Value::Secret(reference) => Some(reference),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        found.extend(
            references
                .into_iter()
                .filter_map(|reference| config.secret(reference))
                .map(|secret| secret.expose().to_string()),
        );
    }
    found
}

mod extractor;
mod job;
mod place;
mod prefix;
mod voices;

pub(crate) use extractor::Extractor;
pub(crate) use job::Job;
pub(crate) use place::place_people;
pub(crate) use prefix::{From, Landings, Read, read};
pub(crate) use voices::{Voice, Voices};

#[cfg(test)]
mod group_tests;
#[cfg(test)]
mod tests;
