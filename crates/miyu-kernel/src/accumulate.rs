//! 流式累积器：模型的输出一段一段地来，拼成完整的内容块（`docs/designs/03-事件模型.md`
//! 第五节「增量和累积器怎么写」）。
//!
//! 驱动把各家的响应分片解码成几种统一的增量（[`Delta`]），累积器照块收下来。正常说完，
//! 拼成一条回复的全部内容（[`Accumulator::finish`]）；被打断，只留收全了的部分
//! （[`Accumulator::cut_off`]）。一次响应一个累积器。

use std::fmt;

use crate::block::{Block, HostedTool, Outcome, Private, Reasoning, Text, ToolCall};
use crate::id::{CallId, Seq};

/// 供应商侧工具的结果最多留多少字节（施工 HT-1，`docs/designs/71-供应商侧工具.md` 第三节第 1 条）：64 KiB。
/// 长的留前面：搜到的条目、读到的正文都是前面要紧。
pub const OUTCOME_BYTES: usize = 64 * 1024;

/// 把供应商侧工具的结果截到 [`OUTCOME_BYTES`] 以内（施工 HT-1）：截在不超过上限的最后一个字的边界上，截了的
/// `cut` 写真。累积器收下时截，推给头的 `model.delta` 也用它，两边是同一份。
#[must_use]
pub fn cut_outcome(mut text: String, failed: bool) -> Outcome {
    let cut = text.len() > OUTCOME_BYTES;
    if cut {
        let mut end = OUTCOME_BYTES;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    Outcome { text, failed, cut }
}

/// 模型响应的一段增量：驱动解码出来的统一写法（`05-内核接口.md` 第七节）。
///
/// 块照开始的先后编号，从 0 数起，一块接一块；几块的字可以交错着来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delta {
    /// 一块开始了。
    Start {
        /// 第几块。
        index: usize,
        /// 这一块是什么。
        kind: Kind,
    },
    /// 这一块的一段字：正文的、思考的，或者工具调用、供应商侧工具参数原文的一段。
    Text {
        /// 第几块。
        index: usize,
        /// 这一段字。
        text: String,
    },
    /// 这一块的私有数据：思考块的签名、供应商自己的调用编号（03 第九节）。
    /// 思考、工具调用、供应商侧工具有，正文没有；一块最多一份。
    Private {
        /// 第几块。
        index: usize,
        /// 数据本身，原样留着。
        private: Private,
    },
    /// 供应商侧工具的结果（施工 HT-1）：只有供应商侧工具有，一块至多一次，在这一块收全以前。累积器收下时截到
    /// [`OUTCOME_BYTES`] 以内（[`cut_outcome`]）。
    Outcome {
        /// 第几块。
        index: usize,
        /// 结果的字，原样；截是累积器的事。
        text: String,
        /// 供应商说这一次跑失败了。
        failed: bool,
    },
    /// 这一块收全了。
    End {
        /// 第几块。
        index: usize,
    },
    /// 回复写到了输出上限，没写完（施工 3-10）：照常说完，内核照它不派里面的工具调用。驱动在收块以后交，至多一次。
    Truncated,
}

/// 一块的种类。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// 正文。
    Text,
    /// 思考。
    Reasoning,
    /// 工具调用。
    ToolCall {
        /// 模型说要调用的工具名。
        name: String,
    },
    /// 供应商自己跑了的一次工具（施工 HT-1）：字是参数原文。
    HostedTool {
        /// 供应商自己的工具名，原样。
        name: String,
    },
}

/// 一次响应的累积器。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Accumulator {
    /// 已经开始的块，照开始的先后。
    blocks: Vec<Building>,
    /// 驱动说写到了输出上限（施工 3-10）。
    truncated: bool,
}

/// 拼到一半的一块。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Building {
    kind: Kind,
    /// 收到的字，照先后接起来。
    text: String,
    private: Option<Private>,
    /// 供应商侧工具的结果，截过的。
    outcome: Option<Outcome>,
    /// 收全了没有。
    ended: bool,
}

impl Accumulator {
    /// 收下一段增量。
    ///
    /// # Errors
    ///
    /// 增量对不上，是驱动的错，返回 [`DeltaError`]，写明是哪一块、哪里对不上：跳过的编号、
    /// 又开始一次的块、还没开始的块、收全以后又来的、给正文块的私有数据、来了两次的私有数据、
    /// 给供应商侧工具以外的块的结果、来了两次的结果。
    pub fn apply(&mut self, delta: Delta) -> Result<(), DeltaError> {
        match delta {
            Delta::Start { index, kind } => {
                if index != self.blocks.len() {
                    let why = if index < self.blocks.len() {
                        "这一块已经开始过了"
                    } else {
                        "跳过了编号，块要一块接一块地开始"
                    };
                    return Err(DeltaError::new(index, why));
                }
                self.blocks.push(Building {
                    kind,
                    text: String::new(),
                    private: None,
                    outcome: None,
                    ended: false,
                });
            }
            Delta::Text { index, text } => self.open(index)?.text.push_str(&text),
            Delta::Private { index, private } => {
                let block = self.open(index)?;
                if block.kind == Kind::Text {
                    return Err(DeltaError::new(index, "正文块没有私有数据"));
                }
                if block.private.is_some() {
                    return Err(DeltaError::new(index, "私有数据来了两次"));
                }
                block.private = Some(private);
            }
            Delta::Outcome {
                index,
                text,
                failed,
            } => {
                let block = self.open(index)?;
                if !matches!(block.kind, Kind::HostedTool { .. }) {
                    return Err(DeltaError::new(index, "只有供应商侧工具有结果"));
                }
                if block.outcome.is_some() {
                    return Err(DeltaError::new(index, "结果来了两次"));
                }
                block.outcome = Some(cut_outcome(text, failed));
            }
            Delta::End { index } => self.open(index)?.ended = true,
            Delta::Truncated => self.truncated = true,
        }
        Ok(())
    }

    /// 驱动说写到了输出上限、没写完（施工 3-10）。
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    /// 到目前为止的每一块（施工 9-8 八补，中途订阅的补推）：是什么、拼好的字、收全了没有、供应商侧工具收到的结果
    /// （施工 HT-1，截过的），照开始的先后。私有数据不给。
    pub fn so_far(&self) -> impl Iterator<Item = (&Kind, &str, bool, Option<&Outcome>)> {
        self.blocks.iter().map(|block| {
            (
                &block.kind,
                block.text.as_str(),
                block.ended,
                block.outcome.as_ref(),
            )
        })
    }

    /// 正常说完：每一块照收到的拼起来。工具调用的编号照这条回复的序号 `reply` 分配，写成
    /// `call_<序号>_<第几个>`；空块不要。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：回复的序号是合法的序号，第几个从 1 数起。
    pub fn finish(self, reply: Seq) -> Vec<Block> {
        unnumbered(self.numbered(reply, true))
    }

    /// 被打断：正文和思考，收到多少留多少；工具调用只留收全了的，没收全的参数不完整，
    /// 执行不了。留下的调用，编号照样一个接一个。供应商侧工具一律留下（施工 HT-1）：供应商那边已经跑了。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：同 [`Accumulator::finish`]。
    pub fn cut_off(self, reply: Seq) -> Vec<Block> {
        unnumbered(self.numbered(reply, false))
    }

    /// 还没收全、可以接着收的第 `index` 块。
    fn open(&mut self, index: usize) -> Result<&mut Building, DeltaError> {
        match self.blocks.get_mut(index) {
            None => Err(DeltaError::new(index, "这一块还没开始")),
            Some(block) if block.ended => Err(DeltaError::new(index, "这一块已经收全了")),
            Some(block) => Ok(block),
        }
    }

    /// 拼成内容块，每一块带着它在流里是第几块（施工 2-3 补）：空块、丢掉的调用不占位置，拼出来的第几块和流里的
    /// 第几块对不上，会话照这个编号把每一块的起止对上落盘的块（`kernel/session.md`「收回复」）。`whole` 是正常
    /// 说完，照 [`Accumulator::finish`] 拼；不是的照 [`Accumulator::cut_off`]，没收全的工具调用丢掉。
    ///
    /// 编号分两趟（施工 HT-1）：先数这次留下的工具调用得 k，工具调用照先后排 `_1.._k`，供应商侧工具照先后接着排
    /// `_k+1` 起。不混着数：旧版本把供应商侧工具当不认识的块，只数工具调用，混着数它就对不上编号、读不进日志。
    pub(crate) fn numbered(self, reply: Seq, whole: bool) -> Vec<(usize, Block)> {
        let kept = |block: &Building| {
            matches!(block.kind, Kind::ToolCall { .. }) && (block.ended || whole)
        };
        let k = self.blocks.iter().filter(|block| kept(block)).count();
        let id = |n: usize| call_id(reply, n);
        let mut out = Vec::new();
        let mut calls = 0;
        let mut hosted = k;
        for (index, block) in self.blocks.into_iter().enumerate() {
            let built = match block.kind {
                Kind::Text if !block.text.is_empty() => Block::Text(Text { text: block.text }),
                Kind::Text => continue,
                Kind::Reasoning if block.text.is_empty() && block.private.is_none() => continue,
                Kind::Reasoning => Block::Reasoning(Reasoning {
                    text: block.text,
                    private: block.private,
                }),
                Kind::ToolCall { name } if block.ended || whole => {
                    calls += 1;
                    Block::ToolCall(ToolCall {
                        call_id: id(calls),
                        name,
                        args: block.text,
                        private: block.private,
                    })
                }
                Kind::ToolCall { .. } => continue,
                Kind::HostedTool { name } => {
                    hosted += 1;
                    Block::HostedTool(HostedTool {
                        call_id: id(hosted),
                        name,
                        args: block.text,
                        outcome: block.outcome,
                        private: block.private,
                    })
                }
            };
            out.push((index, built));
        }
        out
    }
}

/// 丢了工具调用以后重排供应商侧工具的编号（施工 HT-1）：数这一串里还留着的工具调用得 k，供应商侧工具照先后排
/// `_k+1` 起。会话收出错、写到上限的回复时丢掉工具调用（`session/call/settle.rs`），丢了不重排，它的编号就对不上账本。
pub(crate) fn renumber_hosted(reply: Seq, numbered: &mut [(usize, Block)]) {
    let k = numbered
        .iter()
        .filter(|(_, block)| matches!(block, Block::ToolCall(_)))
        .count();
    let hosted = numbered.iter_mut().filter_map(|(_, block)| match block {
        Block::HostedTool(hosted) => Some(hosted),
        _ => None,
    });
    for (n, hosted) in (k + 1..).zip(hosted) {
        hosted.call_id = call_id(reply, n);
    }
}

/// 这条回复里的第 `n` 个调用编号。回复的序号合法，`n` 从 1 数起、不超过块数，造得出来。
fn call_id(reply: Seq, n: usize) -> CallId {
    let n = u32::try_from(n).expect("一条回复的块数放得进 u32");
    CallId::new(reply, n).expect("回复的序号合法，第几个从 1 数起")
}

/// 去掉流里的编号，只留内容块。
fn unnumbered(numbered: Vec<(usize, Block)>) -> Vec<Block> {
    numbered.into_iter().map(|(_, block)| block).collect()
}

/// 增量对不上：驱动的错。这次响应按出错算（施工 2-3）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeltaError {
    /// 第几块。
    pub index: usize,
    /// 哪里对不上。
    pub why: String,
}

impl DeltaError {
    fn new(index: usize, why: &str) -> DeltaError {
        DeltaError {
            index,
            why: why.to_string(),
        }
    }
}

impl fmt::Display for DeltaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "模型的增量对不上，第 {} 块：{}", self.index, self.why)
    }
}

impl std::error::Error for DeltaError {}

#[cfg(test)]
mod tests;
