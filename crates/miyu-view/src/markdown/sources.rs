//! 条目里没有、导出要的原文（调研第四节第 3 条「照序号回事件里取」）：读日志的一方边喂投影边照事件记下，不另读一遍。
//!
//! - 工具结果切过的（条目只带前面 100 行、8 KiB，`output_cut`）：她当时看到的全文，同 `view.detail` 的 `output`；没切过的
//!   条目里已经整份有了，不记。供应商侧工具的照回复里那一块的结果记（施工 HT-3）。
//! - 压缩的摘要：照压缩那一条的编号（`c<替代到的那一条>`）记，同 `view.detail` 的 `summary`；清空的没有摘要，不记。
//! - 回复看到了第几条、自己是第几条（施工 SE-1 中下中）：回复里的块的编号照 `seen` 起（`b<seen>.<块号>`），挑「她现在的
//!   上下文」时照它找回那条 `message.assistant`。
//! - 脚本里调的工具（施工 SE-1 中下中）：了结的 `tool.nested`，照父级记，同 `view.detail` 的嵌套调用列表（[`crate::listed`]）。
//! - 「她现在的上下文」（施工 SE-1 中下中）：要的（[`Sources::keeping_history`]）把每一条交给内核的有效历史（留着一切的那一份，
//!   同载入时从日志重建），喂完 [`Sources::settle`] 落到检查点上，记下检查点和还有效的事件的序号，和请求组装看到的是同一份。
//! - `tools: full` 要的 blob 原文（[`Sources::blobs`]）：读日志的一方照 [`Sources::wanted`] 去 blob 库取。
//! - 派出去的子代理（施工 SE-1 中下下，`children: append`）：派它那一步的结果里记的 `job.started`，任务编号到子会话、标题。

use std::collections::{BTreeMap, BTreeSet};

use miyu_kernel::block::Block as Content;
use miyu_kernel::event::{Body, CompactTrigger, Effect, Event};
use miyu_kernel::history::History;
use miyu_kernel::id::{CallId, ContentHash, JobId, Parent, Seq, SessionId};

use crate::entry::{Body as EntryBody, Entry, EntryId};
use crate::notice::Notice;
use crate::projector::{output_head, output_of};

/// 条目里没有的原文。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sources {
    /// 切过的工具结果的全文：调用编号到她看到的字。
    pub outputs: BTreeMap<CallId, String>,
    /// 压缩的摘要：压缩那一条的编号到摘要原文。
    pub summaries: BTreeMap<EntryId, String>,
    /// 回复：那次请求看到的第几条（`seen`）到这条 `message.assistant` 自己的序号。
    pub replies: BTreeMap<Seq, Seq>,
    /// 脚本里调的工具：父级到挂在它下面了结的 `tool.nested`，照先后。
    pub nested: BTreeMap<Parent, Vec<Event>>,
    /// `tools: full` 要的 blob 原文（照 UTF-8 解，解不开的字换成 U+FFFD）：读日志的一方照 [`Sources::wanted`] 放进来。
    pub blobs: BTreeMap<ContentHash, String>,
    /// 她现在的上下文：[`Sources::settle`] 算好的；没要的、还没算的没有。
    pub context: Option<Context>,
    /// 派出去的子代理：任务编号到子会话和派它时给的标题（`job.started` 的 `session`、`title`）；后台命令不记。
    pub jobs: BTreeMap<JobId, (SessionId, String)>,
    /// 算上下文用的有效历史：留着一切的那一份，喂完落到检查点上。
    history: Option<History>,
}

/// 她现在的上下文：请求组装用的有效历史算出来的。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Context {
    /// 还算数的最近一次压缩替代到哪（检查点那一条的 `upto`）；没压缩过的没有。
    pub upto: Option<Seq>,
    /// 检查点之后还有效的事件的序号。
    pub kept: BTreeSet<Seq>,
}

impl Sources {
    /// 要算「她现在的上下文」的一份（`range: context`）：每一条另交给有效历史。
    #[must_use]
    pub fn keeping_history() -> Sources {
        Sources {
            history: Some(History::whole()),
            ..Sources::default()
        }
    }

    /// 看一条落了盘的事件，要的记下。
    pub fn see(&mut self, event: &Event) {
        match &event.body {
            Body::ToolResult(result) => {
                if let (Some(_), true) = output_head(&result.blocks) {
                    let whole = result
                        .blocks
                        .iter()
                        .filter_map(|block| match block {
                            Content::Text(text) => Some(text.text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<&str>>()
                        .join("\n\n");
                    self.outputs.insert(result.call_id, whole);
                }
                for effect in &result.effects {
                    if let Effect::JobStarted(started) = effect
                        && let Some(session) = &started.session
                    {
                        self.jobs.insert(
                            started.job.clone(),
                            (session.clone(), started.title.clone()),
                        );
                    }
                }
            }
            Body::ContextCompacted(compacted)
                if compacted.trigger != Some(CompactTrigger::Clear) =>
            {
                self.summaries.insert(
                    EntryId::compaction(compacted.upto),
                    compacted.summary.clone(),
                );
            }
            Body::MessageAssistant(reply) => {
                self.replies.insert(reply.seen, event.seq);
                // 供应商侧工具的结果切过的（施工 HT-3）：全文在块里，同 `view.detail` 交的那一份。
                for block in &reply.blocks {
                    if let Content::HostedTool(hosted) = block
                        && let Some(outcome) = &hosted.outcome
                        && output_of(&outcome.text).1
                    {
                        self.outputs.insert(hosted.call_id, outcome.text.clone());
                    }
                }
            }
            Body::ToolNested(nested) => {
                self.nested
                    .entry(nested.id.parent().clone())
                    .or_default()
                    .push(event.clone());
            }
            _ => {}
        }
        if let Some(history) = &mut self.history {
            history.append(light(event));
        }
    }

    /// 喂完了：有效历史落到检查点上，记下检查点和还有效的事件（[`Sources::context`]），有效历史本身放掉。没要上下文的
    /// 什么都不做。
    pub fn settle(&mut self) {
        let Some(mut history) = self.history.take() else {
            return;
        };
        history.settle();
        let upto = history
            .checkpoint()
            .and_then(|checkpoint| match &checkpoint.body {
                Body::ContextCompacted(compacted) => Some(compacted.upto),
                _ => None,
            });
        let kept = history.events().iter().map(|event| event.seq).collect();
        self.context = Some(Context { upto, kept });
    }

    /// `tools: full` 要去 blob 库取的：后台命令的输出、脚本里调的工具交回的结果。`entries` 是要渲染的条目。
    #[must_use]
    pub fn wanted(&self, entries: &[Entry]) -> BTreeSet<ContentHash> {
        let mut wanted: BTreeSet<ContentHash> = entries
            .iter()
            .filter_map(|entry| match &entry.body {
                EntryBody::Notice(Notice::Job(done)) => {
                    done.output.as_ref().map(|output| output.blob.clone())
                }
                _ => None,
            })
            .collect();
        for events in self.nested.values() {
            for event in events {
                if let Body::ToolNested(nested) = &event.body
                    && let Some(result) = &nested.result
                {
                    wanted.insert(result.clone());
                }
            }
        }
        wanted
    }
}

/// 交给有效历史的一份：只用来算哪些还有效，工具结果的内容、改了的文件的原文不要（占内存，算范围用不到）。
fn light(event: &Event) -> Event {
    let mut event = event.clone();
    if let Body::ToolResult(result) = &mut event.body {
        result.blocks.clear();
        for effect in &mut result.effects {
            if let Effect::FileChanged(changed) = effect {
                changed.before_text = None;
                changed.after_text = None;
            }
        }
    }
    event
}
