//! 这时的上下文一块块的（施工 CX-1 上，`protocol.md` 的 `session.context`）：照有效历史组装一次请求，system、每件工具、示范
//! 对话、检查点摘要各估一次，对话里的每条照是谁放进去的归（人说的、她说的、工具的结果，事实照模块和类别），用和压缩线同一个
//! 本地估算器（`estimate.rs`）；用量、锚同 [`Session::context_used`]。只在头问的时候算，只读，不出动作。施工 CX-3 起回放日志
//! 也照同一套算（[`Frame`]、[`breakdown_in`]）：累计用量照每次主请求那一刻的各块摊。

use super::ContextLimits;
use crate::estimate::{self, Flat, Price};
use crate::event::Body;
use crate::history::History;
use crate::id::{FactKind, ModuleId};
use crate::origin::By;
use crate::request::Request;
use crate::request::json;
use crate::session::{Policy, Session};

/// 没有压缩的策略（算不出图、文件的价钱）时，图、文件照这个算：不算，只看字（头也只拿它看个大概）。
const NO_PRICE: Flat = Flat { image: 0, file: 0 };

/// 对话里的一条是谁放进去的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// 人说的（场所里旁听的那几句不进上下文，不算）。
    User,
    /// 她说的：回复、思考、工具调用。
    Assistant,
    /// 工具的结果。
    ToolResult,
    /// 子代理、后台命令交回的回报（施工 CX-2）。
    Report,
    /// 事实：哪个模块注入的（内核自己的没有）、什么类别。记忆模块的 `memory` 是常驻的、`recall` 是召回的。
    Fact {
        /// 模块。
        module: Option<ModuleId>,
        /// 类别。
        kind: FactKind,
    },
}

/// 这时的上下文一块块的，token 数都是本地估的（锚除外）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Breakdown {
    /// 窗口、压缩线，同 [`Session::context_limits`]。
    pub limits: ContextLimits,
    /// 用量，同 [`Session::context_used`]：有锚的是锚加上锚以后的本地估算，没有的整份估；算不了的没有。
    pub used: Option<u64>,
    /// 锚：最近一次主请求时供应商报的（发给的模型和现在的一样才算）。有它，`used` 就是照实测起算的。
    pub anchor: Option<u64>,
    /// system 一整段。
    pub system: u64,
    /// 每件工具的说明和参数格式，照工具面的先后：名字、token 数。
    pub tools: Vec<(String, u64)>,
    /// 示范对话。
    pub demos: u64,
    /// 检查点的摘要；没压过、清空过的是 0。
    pub summary: u64,
    /// 对话：照谁放进去的加起来，照第一次出现的先后；一条都没有的类不列。
    pub said: Vec<(Said, u64)>,
}

impl Breakdown {
    /// 各块本地估的加起来：和 `used` 对不上时，头照 `used / estimated_total` 摊。
    pub fn estimated_total(&self) -> u64 {
        let tools = self.tools.iter().map(|(_, tokens)| tokens).sum::<u64>();
        let said = self.said.iter().map(|(_, tokens)| tokens).sum::<u64>();
        self.system
            .saturating_add(tools)
            .saturating_add(self.demos)
            .saturating_add(self.summary)
            .saturating_add(said)
    }
}

/// 一份策略里不随对话变的几块（施工 CX-3）：system、每件工具、示范对话，照组装一段空的历史估；图、文件照策略里压缩的价钱，
/// 没有压缩的不算图、文件的价钱。回放日志时一份快照算一次。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// system 一整段。
    pub system: u64,
    /// 每件工具，照工具面的先后。
    pub tools: Vec<(String, u64)>,
    /// 示范对话。
    pub demos: u64,
    /// 图、文件的价钱。
    pub price: Flat,
}

impl Frame {
    /// 照策略 `policy` 算。
    pub fn of(policy: &Policy) -> Frame {
        let price = policy
            .compaction
            .as_ref()
            .map_or(NO_PRICE, |compaction| compaction.price);
        Frame {
            price,
            ..Frame::from_request(&policy.assembler.assemble(&History::default()), &price)
        }
    }

    fn from_request(request: &Request, price: &dyn Price) -> Frame {
        Frame {
            system: estimate::text(&request.system),
            tools: request
                .tools
                .iter()
                .map(|tool| {
                    let spec = String::from_utf8_lossy(&json(tool)).into_owned();
                    (tool.name.clone(), estimate::text(&spec))
                })
                .collect(),
            demos: request.messages[..request.stable]
                .iter()
                .map(|message| estimate::message(message, price))
                .sum(),
            price: NO_PRICE,
        }
    }
}

/// 照 `frame` 和一段有效历史 `history` 算各块（施工 CX-3，回放日志用）：限额、锚、用量不算。
pub fn breakdown_in(frame: &Frame, history: &History, price: &dyn Price) -> Breakdown {
    Breakdown {
        limits: ContextLimits {
            window: None,
            compaction_line: None,
        },
        used: None,
        anchor: None,
        system: frame.system,
        tools: frame.tools.clone(),
        demos: frame.demos,
        summary: summary_tokens(history),
        said: spoken(history, price),
    }
}

impl Session {
    /// 这时的上下文一块块的（施工 CX-1 上）：会话 actor 收到头问的时候向内核要一份。
    pub fn breakdown(&self) -> Breakdown {
        let request = self.policy.assembler.assemble(&self.history);
        let priced = self.price();
        let price: &dyn Price = match &priced {
            Some(price) => price,
            None => &NO_PRICE,
        };
        let anchor = self.limits.as_ref().and_then(|limits| {
            estimate::anchor(&self.history)
                .filter(|anchor| anchor.model == limits.model)
                .map(|anchor| anchor.reported)
        });
        Breakdown {
            limits: self.context_limits(),
            used: self.used(&request),
            anchor,
            ..breakdown_in(&Frame::from_request(&request, price), &self.history, price)
        }
    }
}

/// 检查点的摘要：清空的检查点什么都不出（同组装）。
fn summary_tokens(history: &History) -> u64 {
    match history.checkpoint().map(|checkpoint| &checkpoint.body) {
        Some(Body::ContextCompacted(compacted))
            if compacted.trigger != Some(crate::event::CompactTrigger::Clear) =>
        {
            estimate::text(&compacted.summary)
        }
        _ => 0,
    }
}

/// 有效历史里的每条照谁放进去的加起来。
fn spoken(history: &History, price: &dyn Price) -> Vec<(Said, u64)> {
    let mut said: Vec<(Said, u64)> = Vec::new();
    for event in history.ordered() {
        let which = match &event.body {
            // 旁听的场所消息不进上下文（同组装）。
            Body::MessageUser(message)
                if message.venue.as_ref().is_some_and(|venue| venue.ambient) =>
            {
                continue;
            }
            Body::MessageUser(_) => Said::User,
            Body::MessageAssistant(_) => Said::Assistant,
            Body::ToolResult(_) => Said::ToolResult,
            Body::ChildReported(_) | Body::JobReported(_) => Said::Report,
            Body::ContextInjected(fact) => Said::Fact {
                module: match &event.by {
                    By::Module(module) => Some(module.id.clone()),
                    _ => None,
                },
                kind: fact.kind.clone(),
            },
            _ => continue,
        };
        let tokens = estimate::event(event, price);
        match said.iter_mut().find(|(known, _)| *known == which) {
            Some((_, sum)) => *sum = sum.saturating_add(tokens),
            None => said.push((which, tokens)),
        }
    }
    said
}
