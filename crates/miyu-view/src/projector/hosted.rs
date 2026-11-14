//! 供应商侧工具那一步（施工 HT-3，`docs/blueprint/view.md`「种类」的 `tool`、「标题那一句」，设计 `71-供应商侧工具.md`
//! 第三节第 4 条）：模型在供应商那边自己跑的搜索、读网页，画成和核心执行的工具一样的一步，多 `hosted`，标题的 `via` 写是
//! 谁跑的。
//!
//! - 流式：`start` 开条目（准备中），字接参数；这一块的 `end`、下一块开始只算参数收全（在跑）；结果到了出 `output`、定
//!   成了还是出错。结果可以在下一块开始以后才到（驱动把结果攒到后面交），所以收参数时不定状态。
//! - 落盘：照块定稿，流式见过的补齐、纠正，没见过的（翻页）照块开条目；两条路出来的一字不差。没有结果的：回复是打断的
//!   （打断、出错、写到上限）写 `cancelled`，不是的写 `ok`（供应商没报结果）。
//! - 不进「等结果的调用」那张表：没有 `tool.result` 会来，别的调用的结果、确认、提问也找不到它。

use serde_json::Value;

use miyu_kernel::block::{HostedTool, Outcome};
use miyu_kernel::id::CallId;
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;

use super::{Projector, output_of};
use crate::entry::{Body, Entry, EntryId, Tool, ToolState};
use crate::title;

impl Projector {
    /// 写这条回复的供应商给人看的名字：模型写的才有。
    pub(super) fn via_of(&self, by: &By) -> Option<String> {
        match by {
            By::Model(model) => Some(title::via(model.endpoint.as_str(), &self.texts)),
            _ => None,
        }
    }

    /// 流式时一件供应商侧工具开始了：开条目，还在写参数。
    pub(super) fn hosted_step(
        &mut self,
        id: EntryId,
        name: &str,
        via: Option<String>,
        at: Timestamp,
    ) {
        if let Some(running) = self.turn.as_mut() {
            running.spoke = true;
        }
        let group = self.open_group(&id, at);
        let turn = self.turn.as_ref().map(|t| t.id);
        let state = ToolState::Preparing;
        let tool = Tool {
            group: group.clone(),
            call: None,
            name: name.to_string(),
            hosted: true,
            title: title::hosted(name, &Value::Null, state, via, &self.texts),
            state,
            args: String::new(),
            diff: None,
            images: Vec::new(),
            job: None,
            to_title: None,
            approval: None,
            took_ms: None,
            files: Vec::new(),
            skill: None,
            shown: None,
            detail: None,
            output: None,
            output_cut: false,
            questions: Vec::new(),
            nested: None,
            progress: None,
        };
        self.add(Entry {
            id: id.clone(),
            body: Body::Tool(tool),
            turn,
            hidden: false,
            at,
        });
        self.join_group(&group, id);
    }

    /// 参数收全了（这一块的 `end`、下一块开始）：还在写参数的换成在跑，标题照参数写。结果先到了的不动。
    pub(super) fn hosted_ready(&mut self, id: &EntryId) {
        let texts = self.texts.clone();
        self.touch_if(id, |entry| match &mut entry.body {
            Body::Tool(tool) if tool.state == ToolState::Preparing => {
                tool.state = ToolState::Running;
                retitle(tool, &texts);
                true
            }
            _ => false,
        });
        self.refresh_groups_of(vec![id.clone()]);
    }

    /// 流式时结果到了：出 `output`，定成了还是出错。
    pub(super) fn hosted_outcome(&mut self, id: &EntryId, outcome: &Outcome) {
        let texts = self.texts.clone();
        self.touch_if(id, |entry| match &mut entry.body {
            Body::Tool(tool) if tool.hosted => {
                outcome_into(tool, Some(outcome), false);
                retitle(tool, &texts);
                true
            }
            _ => false,
        });
        self.refresh_groups_of(vec![id.clone()]);
    }

    /// 回复落了盘：照块定下这一步，流式时没见过的先开条目。`interrupted` 是这条回复被打断了（打断、出错、写到上限）。
    pub(super) fn settle_hosted(
        &mut self,
        id: &EntryId,
        block: &HostedTool,
        via: Option<String>,
        interrupted: bool,
        at: Timestamp,
    ) {
        if !self.at.contains_key(id) {
            self.hosted_step(id.clone(), &block.name, via.clone(), at);
        }
        let texts = self.texts.clone();
        let call: CallId = block.call_id;
        self.touch_if(id, |entry| {
            let Body::Tool(tool) = &mut entry.body else {
                return false;
            };
            let before = tool.clone();
            tool.call = Some(call);
            tool.name.clone_from(&block.name);
            tool.hosted = true;
            tool.args.clone_from(&block.args);
            outcome_into(tool, block.outcome.as_ref(), interrupted);
            tool.title.via = via;
            retitle(tool, &texts);
            *tool != before
        });
        self.refresh_groups_of(vec![id.clone()]);
    }
}

/// 照结果定状态和 `output`：有结果的照它成了还是出错；没有的落盘时照回复是不是打断的（`cancelled`、`ok`）。
fn outcome_into(tool: &mut Tool, outcome: Option<&Outcome>, interrupted: bool) {
    match outcome {
        Some(outcome) => {
            tool.state = match outcome.failed {
                true => ToolState::Error,
                false => ToolState::Ok,
            };
            (tool.output, tool.output_cut) = output_of(&outcome.text);
        }
        None => {
            tool.state = match interrupted {
                true => ToolState::Cancelled,
                false => ToolState::Ok,
            };
            (tool.output, tool.output_cut) = (None, false);
        }
    }
}

/// 照这一刻的参数、状态重写标题，`via` 照旧。
fn retitle(tool: &mut Tool, texts: &crate::words::Texts) {
    let parsed: Value = serde_json::from_str(&tool.args).unwrap_or(Value::Null);
    let via = tool.title.via.take();
    tool.title = title::hosted(&tool.name, &parsed, tool.state, via, texts);
}
