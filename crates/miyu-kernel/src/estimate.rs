//! 用量和压缩线（`internal/blueprint/compaction.md`「怎么走」第一、二条，`internal/designs/09-压缩.md` Z3，施工 6-1）。
//!
//! 用量 = 锚 + 锚以后新增内容的本地估算。锚是最近一次主请求时供应商报的真实用量（[`anchor`]）；
//! 本地估算照 codex，UTF-8 字节数除以 4（[`message`]）。只估锚之后那一截，估偏了也落在余量里，
//! 下一次请求就被真值盖过去（Claude Code、codex、pi、dsh 都是这个结构）。
//!
//! 这里只有算法；什么时候查、到线怎么压，在会话的 `compaction.rs`（施工 6-2）。压缩留尾巴时，一条事件照同样的数法
//! 估（[`event`]，施工 6-2 下），数的是它进请求以后写成的块（[`Weigh`]，施工 CT-2）。

use crate::assemble::Assembler;
use crate::block::Block;
use crate::event::{Body, CallResult, Event};
use crate::history::History;
use crate::id::Seq;
use crate::origin::Model;
use crate::request::{Message, Request, json};

/// 本地估算：几个 UTF-8 字节算一个 token（照 codex 的 `APPROX_BYTES_PER_TOKEN`，2026-09-29 项目主人定）。
/// 中文一个字三个字节，算成 0.75 个 token，偏多；英文偏少一点，只估锚之后那一截，落在余量里。
/// 回顾的请求照它把 token 的上限折成字节（施工 3-8 四补）。
pub const BYTES_PER_TOKEN: u64 = 4;

/// 一张图在请求里算多少 token：各家的算法不一样，是驱动的事，经模型的限额交进来（施工 6-3 上，DeepSeek 的在
/// `miyu-drivers`）。
pub trait ImagePrice: Send + Sync {
    /// 一张宽 `width`、高 `height` 像素的图。
    fn tokens(&self, width: u32, height: u32) -> u64;
}

/// 一张图、一个文件在请求里算多少 token。
pub trait Price {
    /// 一张宽 `width`、高 `height` 像素的图。
    fn image(&self, width: u32, height: u32) -> u64;
    /// 一个文件。
    fn file(&self) -> u64;
}

/// 没有算法时用的：不管多大，一张图、一个文件都算一个固定的数。
///
/// 出厂是 2000（Claude Code 的做法）：各家从 765 到 2000 都有，取偏多的那一头；真值下一次请求时由
/// 供应商报回来。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flat {
    /// 一张图算多少。
    pub image: u64,
    /// 一个文件算多少。
    pub file: u64,
}

impl Price for Flat {
    fn image(&self, _: u32, _: u32) -> u64 {
        self.image
    }

    fn file(&self) -> u64 {
        self.file
    }
}

/// 驱动交了图片的算法的，图片照它；文件、没交算法的图片照固定的数（施工 6-3 上）。
pub struct WithImages<'a> {
    /// 驱动的图片算法；没有的是 `None`。
    pub images: Option<&'a dyn ImagePrice>,
    /// 固定的数。
    pub flat: Flat,
}

impl Price for WithImages<'_> {
    fn image(&self, width: u32, height: u32) -> u64 {
        match self.images {
            Some(images) => images.tokens(width, height),
            None => self.flat.image,
        }
    }

    fn file(&self) -> u64 {
        self.flat.file
    }
}

/// 锚：最近一次主请求时供应商报的真实用量，和那次请求的样子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    /// 那条 `model.called` 的序号。
    pub seq: Seq,
    /// 发给了哪个端点的哪个模型。
    pub model: Model,
    /// 那次请求有几条消息。它的回复是之后请求里的第 `messages` 条（从 0 数）。
    pub messages: u64,
    /// 那次调用结束时上下文多大：驱动另报了上下文大小（`usage.context`，施工 ACP-1）的照它，没报的是供应商报的
    /// `uncached + cache_read + cache_write + output`。
    pub reported: u64,
}

/// 从有效历史里挑锚：最近一次压缩那一条之后的 `model.called` 中最近的一条，说完了（`ok`）、带着用量、
/// 知道发给了谁，不是辅助请求（施工 3-8 四补：回顾那一次不算）。
///
/// 压缩那一条之前的不算：摘要请求自己那条、尾巴里压缩以前的请求，报的都是压缩前的大小。撤掉的回合里的
/// 已经不在有效历史里了。一条都没有的，没有锚。它后面换过策略快照的（施工 P-1 再补）也没有锚：system、示范对话变了，
/// 它盖不住，整份估到下一次说完。
pub fn anchor(history: &History) -> Option<Anchor> {
    let after = history.checkpoint().map(|checkpoint| checkpoint.seq);
    history
        .events()
        .iter()
        .rev()
        .take_while(|event| after.is_none_or(|after| event.seq > after))
        .take_while(|event| {
            !matches!(&event.body, Body::PolicyChanged(changed) if changed.policy.is_some())
        })
        .find_map(|event| {
            let Body::ModelCalled(called) = &event.body else {
                return None;
            };
            // 回顾这类辅助请求报的是它自己那一次的大小，不是主对话的（施工 3-8 四补）。
            if called.result != CallResult::Ok || called.aside() {
                return None;
            }
            let usage = called.usage.as_ref()?;
            Some(Anchor {
                seq: event.seq,
                model: Model {
                    endpoint: called.endpoint.clone()?,
                    model: called.model.clone()?,
                },
                messages: called.messages,
                // 一次调用里来回请求几次的扩展驱动，四项是几次加起来的，上下文只是最后一次那么大：照它另报的（施工
                // ACP-1，设计 73 第三节；2026-11-23 Antigravity 加起来报、判满判错，主人的群会话停在 too_large）。
                reported: usage.context.unwrap_or_else(|| usage.total()),
            })
        })
}

/// 本地估算和供应商真值的倍数（施工 CT-2，`compaction.md` 第一条第 4 款）：千分之几，夹在 [`SCALE_LEAST`] 到 [`SCALE_MOST`]
/// 之间。字节除以 4 对有的模型系统性偏低（2026-11-27 Antigravity 上的 Gemini：中文约 2.8 字节一个 token，agy 自己还加约
/// 7000，真值是估算的 1.65 到 1.92 倍），尾巴照估算留到 16000，实际是它的三倍多，压完照真值已经过线。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scale(u64);

/// 倍数至少多少（千分之几）：一倍。供应商报得比估算少的不往小里收，照估算（宁多勿少，照 dsh，同 [`usage`] 里锚盖住的那一截）：
/// 往小里收，到线得晚，真超长的就多了。
pub const SCALE_LEAST: u64 = 1000;

/// 估算至少多大的请求才拿来定倍数（施工 CT-2）：供应商自己加的固定开销（agy 约 7000）在小请求上会把倍数放大到顶，不代表
/// 快满时的大请求；和尾巴的预算同一个量级。
pub const SCALE_FROM: u64 = 16_000;

/// 倍数至多多少（千分之几）：真值和估算差得再多，也不信过三倍（一次报错的用量别把尾巴压没了）。
pub const SCALE_MOST: u64 = 3000;

impl Scale {
    /// 一倍：没有可比的时候照估算。
    pub const ONE: Scale = Scale(1000);

    /// 千分之几。
    #[must_use]
    pub fn per_mille(self) -> u64 {
        self.0
    }

    /// 一份本地估算换成真值的口径（向上取整）。
    #[must_use]
    pub fn apply(self, tokens: u64) -> u64 {
        tokens.saturating_mul(self.0).div_ceil(1000)
    }

    /// 真值口径的一个数换成本地估算的口径（留尾巴的预算照它，向下取整）。
    #[must_use]
    pub fn unapply(self, tokens: u64) -> u64 {
        tokens.saturating_mul(1000) / self.0
    }
}

/// 照有效历史挑倍数（至少一倍）：最近一次发给 `model`、说完了、带着用量和估算（`model.called` 的 `estimate`，至少 [`SCALE_FROM`]）的
/// 主请求，真值除以估算。
/// 检查点前面留在尾巴里的也算：压完的头一次请求还没有锚，照它估才不会又一次估少。一条都没有的（新会话、以前的日志）是
/// [`Scale::ONE`]。只读有效历史，重启以后照日志算回来一样。
#[must_use]
pub fn scale(history: &History, model: &Model) -> Scale {
    history
        .events()
        .iter()
        .rev()
        .find_map(|event| {
            let Body::ModelCalled(called) = &event.body else {
                return None;
            };
            if called.result != CallResult::Ok || called.aside() {
                return None;
            }
            if called.endpoint.as_ref() != Some(&model.endpoint)
                || called.model.as_ref() != Some(&model.model)
            {
                return None;
            }
            let estimate = called.estimate.filter(|estimate| *estimate >= SCALE_FROM)?;
            let usage = called.usage.as_ref()?;
            let reported = usage.context.unwrap_or_else(|| usage.total());
            Some(Scale(
                reported
                    .saturating_mul(1000)
                    .checked_div(estimate)?
                    .clamp(SCALE_LEAST, SCALE_MOST),
            ))
        })
        .unwrap_or(Scale::ONE)
}

/// 这一份请求整份的本地估算，不带锚、不乘倍数（施工 CT-2）：发主请求时记进 `model.called` 的 `estimate`，和真值配对。
#[must_use]
pub fn whole(request: &Request, price: &dyn Price) -> u64 {
    head(request, request.messages.len(), price)
}

/// 这一次请求发给 `model` 时，上下文用了多少 token：锚加上锚以后新增的那几条的估算，估算都乘上 `scale`（施工 CT-2）。
///
/// - 锚盖住的是工具面、system 和前 `messages + 1` 条（锚那次请求加上它的回复）：请求只追加，它们和锚那一次
///   一样。本地对它们的估算（乘过倍数）比供应商报的还大的，取估算（照 dsh，宁多勿少）。
/// - 整份估：没有锚；锚发给的端点、模型和这一次不一样（换过模型，照 dsh）；这一次的消息没比锚多（不该出现，
///   兜底）。
pub fn usage(
    request: &Request,
    anchor: Option<&Anchor>,
    model: &Model,
    price: &dyn Price,
    scale: Scale,
) -> u64 {
    let covered = anchor
        .filter(|anchor| &anchor.model == model)
        .and_then(|anchor| {
            let covered = usize::try_from(anchor.messages).ok()?.checked_add(1)?;
            (covered <= request.messages.len()).then_some((anchor.reported, covered))
        });
    let Some((reported, covered)) = covered else {
        return scale.apply(head(request, request.messages.len(), price));
    };
    let added = request.messages[covered..]
        .iter()
        .fold(0u64, |sum, m| sum.saturating_add(message(m, price)));
    reported
        .max(scale.apply(head(request, covered, price)))
        .saturating_add(scale.apply(added))
}

/// 输出预留：min(模型的最大输出, `reserve_cap`)；没报最大输出的，按 `reserve_cap`（`compaction.md` 第二条第 2 条）。
pub fn reserve(max_output: Option<u64>, reserve_cap: u64) -> u64 {
    max_output.map_or(reserve_cap, |max| max.min(reserve_cap))
}

/// 一条消息的本地估算：文字、思考、工具调用的名字和参数、驱动的私有数据、认不出的块、只有路径的附件，UTF-8 字节加起来除以 4，
/// 向上取整；图片、文件照 `price`。不另加每块、每条的结构开销。
pub fn message(message: &Message, price: &dyn Price) -> u64 {
    let blocks = match message {
        Message::User { blocks } | Message::Assistant { blocks } | Message::Tool { blocks, .. } => {
            blocks
        }
    };
    blocks_of(blocks, price)
}

/// 一段字的本地估算：UTF-8 字节除以 4，向上取整。压后重读的文件照它算（施工 6-5）。
pub fn text(text: &str) -> u64 {
    tokens(text.len())
}

/// 一条事件的本地估算，留尾巴时用（施工 6-2 下）：人的消息、回复、工具结果照它们的内容块，和 [`message`] 一样数；
/// 事实照它的原文；别的事件不进上下文，算 0。
pub fn event(event: &Event, price: &dyn Price) -> u64 {
    match &event.body {
        Body::MessageUser(message) => blocks_of(&message.blocks, price),
        Body::MessageAssistant(reply) => blocks_of(&reply.blocks, price),
        Body::ToolResult(result) => blocks_of(&result.blocks, price),
        Body::ContextInjected(fact) => tokens(fact.text.len()),
        // 回报（施工 CX-2）：组装时子代理的正文、后台命令带回来的最后一段原样放进请求，外面的开头、收尾不算。
        Body::ChildReported(reported) => tokens(reported.text.len()),
        Body::JobReported(reported) => tokens(reported.tail.as_ref().map_or(0, String::len)),
        // 监视的命令出的行（施工 MN-1 上）：这一批原样放进请求。
        Body::JobLines(lines) => tokens(lines.text.len()),
        // 人打的命令的输出（施工 SH-1 上）：和那句话拼在一起原样放进请求。
        Body::ShellRan(ran) => tokens(ran.text.len()),
        _ => 0,
    }
}

/// 一条事件进请求以后的估算（施工 CT-2）：留尾巴、找最大的那一条、截短重试都照它。组装器交了它写成的块的（群里的一行带着
/// 行首、名字、编号，群聊近况里的一行，[`Assembler::rendered`]）照那些块数；没交的照它自己的内容块（[`event`]）。
pub struct Weigh<'a> {
    /// 这个会话的组装器。
    pub assembler: &'a dyn Assembler,
    /// 这条事件所在的有效历史。
    pub history: &'a History,
    /// 图片、文件的价钱。
    pub price: &'a dyn Price,
}

impl Weigh<'_> {
    /// 一条事件。
    pub fn event(&self, event: &Event) -> u64 {
        match self.assembler.rendered(self.history, event) {
            Some(blocks) => blocks_of(&blocks, self.price),
            None => self::event(event, self.price),
        }
    }
}

/// 一串内容块的估算：字节加起来除以 4，向上取整；图片、文件照 `price`。
fn blocks_of(blocks: &[Block], price: &dyn Price) -> u64 {
    let mut bytes = 0usize;
    let mut media = 0u64;
    for block in blocks {
        match block {
            Block::Text(text) => bytes = bytes.saturating_add(text.text.len()),
            Block::Reasoning(reasoning) => {
                bytes = bytes.saturating_add(reasoning.text.len());
                if let Some(private) = &reasoning.private {
                    bytes = bytes.saturating_add(private.data.get().len());
                }
            }
            Block::ToolCall(call) => {
                bytes = bytes
                    .saturating_add(call.name.len())
                    .saturating_add(call.args.len());
                if let Some(private) = &call.private {
                    bytes = bytes.saturating_add(private.data.get().len());
                }
            }
            // 供应商侧工具（施工 HT-1）：工具名、参数、结果的字、私有数据，同工具调用的算法。
            Block::HostedTool(hosted) => {
                bytes = bytes
                    .saturating_add(hosted.name.len())
                    .saturating_add(hosted.args.len());
                if let Some(outcome) = &hosted.outcome {
                    bytes = bytes.saturating_add(outcome.text.len());
                }
                if let Some(private) = &hosted.private {
                    bytes = bytes.saturating_add(private.data.get().len());
                }
            }
            Block::Unknown(raw) => bytes = bytes.saturating_add(raw.get().len()),
            Block::Image(image) => {
                media = media.saturating_add(price.image(image.width, image.height))
            }
            Block::File(_) => media = media.saturating_add(price.file()),
            // 只有路径的附件（施工 AT-1）：请求里是一句带名字、路径、大小的话，照这一块规范的 JSON 算，和那一句差不多长；
            // 旧版本把它当认不出的块，也是这么算。
            Block::Path(_) => bytes = bytes.saturating_add(json(block).len()),
        }
    }
    tokens(bytes).saturating_add(media)
}

/// 整份请求里工具面、system 和前 `messages` 条消息的估算。工具面照它规范的 JSON 字节；没有工具的，
/// 工具面不发，算 0。
fn head(request: &Request, messages: usize, price: &dyn Price) -> u64 {
    let face = if request.tools.is_empty() {
        0
    } else {
        tokens(json(&request.tools).len())
    };
    request.messages[..messages].iter().fold(
        face.saturating_add(tokens(request.system.len())),
        |sum, m| sum.saturating_add(message(m, price)),
    )
}

/// 几个字节算几个 token：除以 4，向上取整。
fn tokens(bytes: usize) -> u64 {
    u64::try_from(bytes)
        .unwrap_or(u64::MAX)
        .div_ceil(BYTES_PER_TOKEN)
}

#[cfg(test)]
mod tests;
