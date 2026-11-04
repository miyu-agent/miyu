//! 编好的历史按条留着（施工 V-42）：同一个会话前后两次请求，前面那一大段历史一个字都没变，用不着每次都重新转成 JSON。
//!
//! 一个会话一份 [`Memo`]，主请求编码时交给驱动（[`crate::Driver::fragments_memo`]）。里面留着：上一次编过的每一条消息
//! （原样的一份，拿来逐字比）、它编出来的那一截（片段，图片、PDF 照旧只记 blob）、编它时查过的调用编号和用过的 blob；
//! 再加 system、工具面编好的那一截。下一次编码：
//!
//! 1. 设置（驱动家族、开关、占位的几句、模型能收什么、图的转述）有一样不同，整份扔掉，从头编。
//! 2. 消息从第一条不一样的起扔掉（压缩、撤销、改写、事实块插进去，都落在这里），只比原样的那一份，不靠哈希。
//! 3. 留着的消息里，查过的调用编号这一次对出来的不一样（后面的消息带来了同一个编号），或者用的 blob 这一次没取到、大小
//!    不一样，从那一条起扔掉，重新编（编不了的照旧报缺了哪一个）。
//! 4. 剩下的接着用，后面的照常编、留下。
//!
//! 留着的只是「一条消息自己编出来的那一截」。和前后有关的照旧每次现排：openai-chat 工具结果里挪出来的图片另起的那条 user、
//! 接着写时不发的那句和加了字段的那一条，anthropic 相邻同角色合成一条、缓存打点，各家的头尾（模型名、输出上限、温度、思考
//! 强度、工具面放在哪）。宁可多扔，不发错一个字节：拿整份编的（[`crate::Driver::fragments`]）当准绳，逐字节对过。
//!
//! 只在内存里：会话退下就放，不落盘。多占的大约是上下文那么大的两份（原样的消息、编好的字）。

use std::collections::BTreeMap;

use miyu_kernel::block::{Block, ToolCall};
use miyu_kernel::id::{CallId, ContentHash};
use miyu_kernel::request::Message;

use crate::media::is_pdf;
use crate::openai_chat::Compat;
use crate::{BlobBytes, Call, DriverTexts, Inputs, anthropic, openai_chat, openai_responses};

/// 一个会话留着的编好的历史（施工 V-42，见模块的说明）。造一个空的，主请求编码时交给驱动；驱动自己往里放、自己认。
#[derive(Debug, Default)]
pub struct Memo {
    /// 驱动家族都认的那几样。
    pub(crate) core: Core,
    /// 各驱动自己留的。
    pub(crate) slot: Slot,
}

impl Memo {
    /// 空的。
    pub fn new() -> Memo {
        Memo::default()
    }

    /// 扔掉留着的一切（编码出错、锁坏了的时候）。
    pub fn clear(&mut self) {
        *self = Memo::default();
    }

    /// 留着几条消息（测试、量尺看）。
    pub fn len(&self) -> usize {
        self.core.messages.len()
    }

    /// 一条都没留。
    pub fn is_empty(&self) -> bool {
        self.core.messages.is_empty()
    }

    /// 上一次编码前面接着用了几条（测试、量尺看：只往后加的，应该是上一次留着的那么多）。
    pub fn reused(&self) -> usize {
        self.core.reused
    }

    /// 照这一次的设置 `setup`、要编的消息 `messages`、取好的 `blobs` 定前几条接着用（见模块的说明第 1 到 3 条），驱动自己
    /// 留的那份跟着截短。交回接着用几条；之后的调用编号对照（[`Core::ids`]）已经是这一次整份的。
    pub(crate) fn reuse(
        &mut self,
        setup: &Setup<'_>,
        messages: &[Message],
        call: &Call,
        blobs: &dyn BlobBytes,
        provider: fn(&ToolCall) -> Option<String>,
    ) -> usize {
        if !self
            .core
            .setup
            .as_ref()
            .is_some_and(|kept| setup.same(kept))
        {
            self.clear();
            self.core.setup = Some(setup.owned());
        }
        let same = messages
            .iter()
            .zip(&self.core.messages)
            .take_while(|(now, then)| now == then)
            .count();
        self.core.truncate(same);
        let mut reused = same;
        self.core.extend_ids(&messages[reused..], provider);
        if let Some(stale) = self.core.stale(call, blobs) {
            self.core.truncate(stale);
            self.core.extend_ids(&messages[stale..], provider);
            reused = stale;
        }
        self.slot.truncate(reused);
        self.core.reused = reused;
        reused
    }
}

/// 驱动家族都认的那几样。
#[derive(Debug, Default)]
pub(crate) struct Core {
    /// 编的时候照的设置。
    setup: Option<Owned>,
    /// 留着的消息，原样的一份。
    messages: Vec<Message>,
    /// 每条消息编的时候查过的调用编号、用过的 blob，和 `messages` 一一对应。
    bases: Vec<Basis>,
    /// 内核的调用编号到线上的编号：`bases` 里各条带来的照先后叠起来，再叠上这一次后面那几条的（[`Memo::reuse`] 以后）。
    ids: BTreeMap<CallId, String>,
    /// 叠进 `ids` 的有没有 `bases` 以外的（这一次后面那几条的）：截短时要重新叠。
    pending: bool,
    /// 上一次编码前面接着用了几条。
    reused: usize,
}

impl Core {
    /// 内核的调用编号到线上的编号：这一次整份的（[`Memo::reuse`] 以后）。
    pub(crate) fn ids(&self) -> &BTreeMap<CallId, String> {
        &self.ids
    }

    /// 只留前 `len` 条；调用编号对照照留下的重新叠。
    fn truncate(&mut self, len: usize) {
        if len < self.bases.len() || self.pending {
            let dropped = self
                .bases
                .drain(len..)
                .any(|basis| !basis.provides.is_empty());
            self.messages.truncate(len);
            if dropped || self.pending {
                self.ids.clear();
                for basis in &self.bases {
                    for (call_id, id) in &basis.provides {
                        self.ids.insert(*call_id, id.clone());
                    }
                }
            }
            self.pending = false;
        }
    }

    /// 后面要编的那几条带来的调用编号叠上去。
    fn extend_ids(&mut self, messages: &[Message], provider: fn(&ToolCall) -> Option<String>) {
        for message in messages {
            for (call_id, id) in provides(message, provider) {
                self.ids.insert(call_id, id);
                self.pending = true;
            }
        }
    }

    /// 留着的消息里第一条不能接着用的：查过的调用编号这一次对出来的不一样，或者用的 blob 这一次没取到、大小不一样。
    fn stale(&self, call: &Call, blobs: &dyn BlobBytes) -> Option<usize> {
        self.bases.iter().position(|basis| {
            basis
                .uses
                .iter()
                .any(|(call_id, id)| self.ids.get(call_id) != id.as_ref())
                || basis.blobs.iter().any(|held| !held.still(blobs))
                || basis.inputs != call.inputs
        })
    }

    /// 接着留下这几条：原样的一份、查过的调用编号（照这一次整份的对照）、用过的 blob。各驱动编好的那一截自己留。
    pub(crate) fn extend(&mut self, messages: &[Message], call: &Call, blobs: &dyn BlobBytes) {
        for message in messages {
            self.bases.push(Basis {
                provides: Vec::new(),
                uses: uses(message)
                    .map(|call_id| (*call_id, self.ids.get(call_id).cloned()))
                    .collect(),
                blobs: held(message, call.inputs, blobs),
                inputs: call.inputs,
            });
            self.messages.push(message.clone());
        }
    }

    /// 留下的这几条带来的调用编号记进各自的底子（[`Core::extend`] 以后；`provider` 同 [`Memo::reuse`]）：以后截短时照它重新叠。
    pub(crate) fn settle(&mut self, from: usize, provider: fn(&ToolCall) -> Option<String>) {
        for (basis, message) in self.bases[from..].iter_mut().zip(&self.messages[from..]) {
            basis.provides = provides(message, provider);
        }
        self.pending = false;
    }
}

/// 一条消息编的时候查过、用过的：下一次照它认还能不能接着用。
#[derive(Debug)]
struct Basis {
    /// 这一条带来的调用编号（assistant 里的工具调用，私有数据是这个驱动的）。
    provides: Vec<(CallId, String)>,
    /// 这一条查过的调用编号，和当时对出来的（没有的照内核的编号写）。
    uses: Vec<(CallId, Option<String>)>,
    /// 这一条用过的 blob。
    blobs: Vec<Held>,
    /// 编的时候模型能收什么（设置里已经有，这里再守一道）。
    inputs: Inputs,
}

/// 用过的一个 blob：照字放进消息的要整份的字节，照原样发的只要大小。
#[derive(Debug)]
struct Held {
    blob: ContentHash,
    /// 当时的字节数。
    size: u64,
    /// 要整份的字节。
    whole: bool,
}

impl Held {
    /// 这一次取好的里面还有，大小一样。
    fn still(&self, blobs: &dyn BlobBytes) -> bool {
        measure(blobs, &self.blob, self.whole) == Some(self.size)
    }
}

/// 取好的里面这个 blob 有多大：要整份的照字节量，别的照大小；没有的没有。
fn measure(blobs: &dyn BlobBytes, blob: &ContentHash, whole: bool) -> Option<u64> {
    match whole {
        true => blobs
            .bytes(blob)
            .map(|bytes| u64::try_from(bytes.len()).unwrap_or(u64::MAX)),
        false => blobs.size(blob),
    }
}

/// 一条消息带来的调用编号：assistant 里的工具调用，照 `provider` 认出线上的编号的。照先后。
fn provides(message: &Message, provider: fn(&ToolCall) -> Option<String>) -> Vec<(CallId, String)> {
    let Message::Assistant { blocks } = message else {
        return Vec::new();
    };
    blocks
        .iter()
        .filter_map(|block| match block {
            Block::ToolCall(call) => provider(call).map(|id| (call.call_id, id)),
            _ => None,
        })
        .collect()
}

/// 一条消息编码时会查的调用编号：assistant 里工具调用的，工具结果的。
fn uses(message: &Message) -> impl Iterator<Item = &CallId> {
    let (calls, result) = match message {
        Message::Assistant { blocks } => (blocks.as_slice(), None),
        Message::Tool { call_id, .. } => (&[][..], Some(call_id)),
        Message::User { .. } => (&[][..], None),
    };
    calls
        .iter()
        .filter_map(|block| match block {
            Block::ToolCall(call) => Some(&call.call_id),
            _ => None,
        })
        .chain(result)
}

/// 一条消息编码时会用的 blob（照 [`crate::media::blobs_needed`]、[`crate::media::blobs_whole`] 的认法）。当时就没取到、编码却成了
/// 的（anthropic 不收的图片类型换成了字），编出来的和它无关，不记。
fn held(message: &Message, inputs: Inputs, blobs: &dyn BlobBytes) -> Vec<Held> {
    let blocks = match message {
        Message::User { blocks } | Message::Tool { blocks, .. } => blocks,
        Message::Assistant { .. } => return Vec::new(),
    };
    blocks
        .iter()
        .filter_map(|block| match block {
            Block::Image(image) if inputs.images => Some((&image.blob, false)),
            Block::File(file) => Some((&file.blob, !(inputs.pdf && is_pdf(file)))),
            _ => None,
        })
        .filter_map(|(blob, whole)| {
            Some(Held {
                blob: blob.clone(),
                size: measure(blobs, blob, whole)?,
                whole,
            })
        })
        .collect()
}

/// 编码时照的设置（借着的一份）：驱动家族、开关（openai-chat 的）、占位的几句、模型能收什么、图的转述。有一样和留着的不同，
/// 留着的整份扔掉。
pub(crate) struct Setup<'a> {
    pub(crate) family: &'static str,
    pub(crate) compat: Option<&'a Compat>,
    pub(crate) texts: &'a DriverTexts,
    pub(crate) inputs: Inputs,
    pub(crate) described: &'a BTreeMap<ContentHash, String>,
}

/// 留着的那一份设置。
#[derive(Debug)]
struct Owned {
    family: &'static str,
    compat: Option<Compat>,
    texts: DriverTexts,
    inputs: Inputs,
    described: BTreeMap<ContentHash, String>,
}

impl Setup<'_> {
    fn same(&self, kept: &Owned) -> bool {
        self.family == kept.family
            && self.compat == kept.compat.as_ref()
            && *self.texts == kept.texts
            && self.inputs == kept.inputs
            && *self.described == kept.described
    }

    fn owned(&self) -> Owned {
        Owned {
            family: self.family,
            compat: self.compat.cloned(),
            texts: self.texts.clone(),
            inputs: self.inputs,
            described: self.described.clone(),
        }
    }
}

/// 各驱动自己留的：编好的每一条，加上 system、工具面那一截。
#[derive(Debug, Default)]
pub(crate) enum Slot {
    /// 还没有。
    #[default]
    None,
    /// openai-chat 的。
    Chat(openai_chat::Kept),
    /// anthropic 的。
    Anthropic(anthropic::Kept),
    /// openai-responses 的。
    Responses(openai_responses::Kept),
}

impl Slot {
    /// 编好的每一条只留前 `len` 条。
    fn truncate(&mut self, len: usize) {
        match self {
            Slot::None => {}
            Slot::Chat(kept) => kept.entries.truncate(len),
            Slot::Anthropic(kept) => kept.entries.truncate(len),
            Slot::Responses(kept) => kept.entries.truncate(len),
        }
    }
}
