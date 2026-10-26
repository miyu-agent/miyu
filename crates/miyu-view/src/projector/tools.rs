//! 调工具的那一步（终端蓝图 `tui.md`「时间线」第 3、6–9、12、13 条）：还在写参数是「准备……」；参数写完照它写标题、
//! 估加减的行数；结果到了换成结果的状态、结果那一句、真的行数、结果里的图；派出去的后台任务记下来，了结时照它写。
//! 要人确认的记在这一步上；一组题答了另起一条旁白。

use std::collections::BTreeMap;

use serde_json::Value;

use miyu_kernel::block::Block as Content;
use miyu_kernel::event::{
    ApprovalDecided, ApprovalRequested, Body as EventBody, Effect, Event, QuestionAnswered,
    QuestionAsked, ToolResult,
};
use miyu_kernel::id::CallId;
use miyu_kernel::time::Timestamp;

use super::{Job, Projector};
use crate::entry::{
    Answered, Approval, Body, Diff, Entry, EntryId, Picture, SkillUsed, Tool, ToolState, TouchKind,
    Touched,
};
use crate::estimate;
use crate::notice::Notice;
use crate::summary::Counted;
use crate::title;
use crate::words::{Texts, ToolKind};

/// 一步工具结果里收起那一行要的（施工 X-24）：发出去了几张图、英文的显示名（结果里记下了的）。
#[derive(Debug, Clone, Default)]
pub(super) struct Sent {
    count: usize,
    english: Option<String>,
}

/// 一步在收起那一行里算什么。
pub(super) fn counted(entry: &Entry, texts: &Texts, sent: Option<&Sent>) -> Counted {
    match &entry.body {
        Body::Tool(tool) => {
            let args: Value = serde_json::from_str(&tool.args).unwrap_or(Value::Null);
            let arg = |key: &str| args.get(key).and_then(Value::as_str).map(str::to_string);
            Counted::Tool {
                kind: texts.kinds.of(&tool.name),
                failed: tool.state.failed(),
                diff: tool.diff.map_or((0, 0), |d| (d.added, d.removed)),
                title: arg("description"),
                to_session: arg("to")
                    .is_some_and(|to| miyu_kernel::id::SessionId::parse(&to).is_ok()),
                package: texts.packages.get(&tool.name).cloned(),
                sent: sent.map_or(0, |sent| sent.count),
                names: (
                    tool.title.name.clone(),
                    sent.and_then(|sent| sent.english.clone())
                        .or_else(|| texts.english.face(&tool.name).map(|face| face.name))
                        .unwrap_or_else(|| tool.name.clone()),
                ),
            }
        }
        Body::Thought(thought) => Counted::Thought(thought.took_ms.unwrap_or(0)),
        _ => Counted::Thought(0),
    }
}

impl Projector {
    /// 调工具的一步记进在进行的那一段。`call` 是调用编号，流式时还没有。
    pub(super) fn tool_step(
        &mut self,
        id: EntryId,
        name: &str,
        call: Option<CallId>,
        args: String,
        at: Timestamp,
    ) {
        if let Some(running) = self.turn.as_mut() {
            running.spoke = true;
        }
        let group = self.open_group(&id, at);
        let turn = self.turn.as_ref().map(|t| t.id);
        let state = match call {
            Some(_) => ToolState::Running,
            None => ToolState::Preparing,
        };
        let parsed: Value = serde_json::from_str(&args).unwrap_or(Value::Null);
        let tool = Tool {
            group: group.clone(),
            call,
            name: name.to_string(),
            title: title::of(name, &parsed, state, None, &BTreeMap::new(), &self.texts),
            state,
            diff: self.estimate(name, &parsed, state),
            args,
            images: Vec::new(),
            job: None,
            to_title: self.to_title(name, &parsed),
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
        if let Some(call) = call {
            self.calls.insert(call, id.clone());
        }
        self.add(Entry {
            id: id.clone(),
            body: Body::Tool(tool),
            turn,
            hidden: false,
            at,
        });
        self.join_group(&group, id);
    }

    /// 参数写完了：照它写标题、估行数，等结果。
    pub(super) fn ready(&mut self, id: &EntryId) {
        let texts = self.texts.clone();
        let mut estimate = None;
        let mut to_title = None;
        if let Some(Body::Tool(tool)) = self.get(id).map(|e| &e.body)
            && tool.state == ToolState::Preparing
        {
            let parsed: Value = serde_json::from_str(&tool.args).unwrap_or(Value::Null);
            estimate = Some(self.estimate(&tool.name, &parsed, ToolState::Running));
            to_title = Some(self.to_title(&tool.name, &parsed));
        }
        let (Some(diff), Some(to_title)) = (estimate, to_title) else {
            return;
        };
        self.touch(id, |entry| {
            if let Body::Tool(tool) = &mut entry.body {
                let parsed: Value = serde_json::from_str(&tool.args).unwrap_or(Value::Null);
                tool.state = ToolState::Running;
                tool.title = title::of(
                    &tool.name,
                    &parsed,
                    tool.state,
                    None,
                    &BTreeMap::new(),
                    &texts,
                );
                tool.diff = diff;
                tool.to_title = to_title;
            }
        });
        self.refresh_groups_of(vec![id.clone()]);
    }

    /// 回复落了盘：这一步配上调用编号，参数照落了盘的。
    pub(super) fn named(&mut self, id: &EntryId, call: CallId, args: &str) {
        self.calls.insert(call, id.clone());
        let needs = matches!(self.get(id).map(|e| &e.body), Some(Body::Tool(tool)) if tool.call != Some(call) || tool.args != args);
        if needs {
            let args = args.to_string();
            self.touch(id, |entry| {
                if let Body::Tool(tool) = &mut entry.body {
                    tool.call = Some(call);
                    tool.args = args;
                }
            });
        }
        self.ready(id);
    }

    /// 编辑、写入照参数估的行数；别的工具、还在写参数的没有。
    fn estimate(&self, name: &str, args: &Value, state: ToolState) -> Option<Diff> {
        (state != ToolState::Preparing && self.texts.kinds.of(name) == Some(ToolKind::Edit))
            .then(|| estimate::from_args(args))
            .flatten()
    }

    /// 留言发给子代理的：那个子代理的标题。
    fn to_title(&self, name: &str, args: &Value) -> Option<String> {
        if self.texts.kinds.of(name) != Some(ToolKind::Message) {
            return None;
        }
        let to = args.get("to").and_then(Value::as_str)?;
        let job = miyu_kernel::id::JobId::parse(to).ok()?;
        self.jobs.get(&job).map(|j| j.title.clone())
    }

    /// 一次调用的结果。
    /// 翻页时这一页之前的日志：派出去的后台任务记下标题、命令（命令照派它那次调用的参数），别的都不看。
    pub(super) fn learn_jobs(&mut self, earlier: &[Event]) {
        let mut commands: BTreeMap<CallId, Option<String>> = BTreeMap::new();
        for event in earlier {
            match &event.body {
                EventBody::MessageAssistant(reply) => {
                    for block in &reply.blocks {
                        if let Content::ToolCall(call) = block {
                            commands.insert(call.call_id, command_of(&call.args));
                        }
                    }
                }
                EventBody::ToolResult(result) => {
                    for effect in &result.effects {
                        if let Effect::JobStarted(started) = effect {
                            let command = commands.get(&result.call_id).cloned().flatten();
                            self.jobs.insert(
                                started.job.clone(),
                                Job {
                                    what: started.what.clone(),
                                    title: started.title.clone(),
                                    command,
                                },
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub(super) fn result(&mut self, event: &Event, result: &ToolResult) {
        let Some(id) = self.calls.get(&result.call_id).cloned() else {
            return;
        };
        let state = ToolState::of(&result.status);
        let texts = self.texts.clone();
        let real = self.real_lines(result);
        let images: Vec<Picture> = result
            .blocks
            .iter()
            .filter_map(|block| match block {
                Content::Image(image) => Some(Picture {
                    blob: image.blob.clone(),
                    media_type: image.media_type.as_str().to_string(),
                    width: image.width,
                    height: image.height,
                }),
                _ => None,
            })
            .collect();
        let files: Vec<Touched> = result
            .effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::FileChanged(changed) => Some(Touched {
                    path: changed.path.clone(),
                    // 改之前没有的是新建的（施工 9-8 三补，网页要分「新建」「修改」）。
                    action: match changed.before {
                        Some(_) => TouchKind::Changed,
                        None => TouchKind::Created,
                    },
                }),
                Effect::FileTrashed(trashed) => Some(Touched {
                    path: trashed.path.clone(),
                    action: TouchKind::Trashed,
                }),
                _ => None,
            })
            .collect();
        // 读的是技能的 `SKILL.md`（施工 9-8 四补）：名字照效果；给人看的名字照读的那一刻记下的（施工 9-8 五补），以前的日志
        // 没记的照交进来的字，再没有的照名字。
        let skill = result.effects.iter().find_map(|effect| match effect {
            Effect::FileRead(read) => read.skill.as_ref().map(|name| SkillUsed {
                name: name.clone(),
                title: crate::words::pick(&read.display_name, &texts.language)
                    .or_else(|| texts.skills.get(name).cloned())
                    .unwrap_or_else(|| name.clone()),
            }),
            _ => None,
        });
        // 给人看的文件（施工 D-5）。
        let shown = result.effects.iter().find_map(|effect| match effect {
            Effect::FileShown(shown) => Some(shown.clone()),
            _ => None,
        });
        let mut job = None;
        for effect in &result.effects {
            if let Effect::JobStarted(started) = effect {
                job = Some(started.job.clone());
                let command = self.get(&id).and_then(|e| match &e.body {
                    Body::Tool(tool) => command_of(&tool.args),
                    _ => None,
                });
                self.jobs.insert(
                    started.job.clone(),
                    Job {
                        what: started.what.clone(),
                        title: started.title.clone(),
                        command,
                    },
                );
            }
        }
        let (output, output_cut) = output_head(&result.blocks);
        // 等人确认的那一段不算（施工 9-8 十四补）。
        let took = self
            .get(&id)
            .map_or(0, |entry| self.busy_ms(entry.at, event.at));
        self.ended.insert(id.clone(), event.at);
        self.sent.insert(
            id.clone(),
            Sent {
                count: result.sent.len(),
                english: crate::words::pick(&result.display_name, "en"),
            },
        );
        self.touch(&id, |entry| {
            if let Body::Tool(tool) = &mut entry.body {
                let parsed: Value = serde_json::from_str(&tool.args).unwrap_or(Value::Null);
                tool.state = state;
                tool.title = title::of(
                    &tool.name,
                    &parsed,
                    state,
                    result.human.as_ref(),
                    &result.display_name,
                    &texts,
                );
                tool.images = images;
                tool.job = job;
                tool.files = files;
                tool.skill = skill;
                tool.shown = shown;
                tool.detail = result.detail.clone();
                tool.output = output;
                tool.output_cut = output_cut;
                if let Some(real) = real
                    && !state.failed()
                {
                    tool.diff = Some(real);
                }
                tool.took_ms = Some(took);
                tool.progress = None;
            }
        });
        // 结果到了，进度去掉（施工 X-45 补）：攒着的半行、压着的值一起扔掉。
        self.progress_over(result.call_id);
        self.refresh_groups_of(vec![id]);
    }

    /// 改了的文件真加减了几行：有端口、效果里有改过的文件才算得出。
    fn real_lines(&self, result: &ToolResult) -> Option<Diff> {
        let lines = self.lines.as_ref()?;
        let mut total: Option<(u64, u64)> = None;
        for effect in &result.effects {
            if let Effect::FileChanged(changed) = effect {
                let (added, removed) = lines.count(changed)?;
                let (a, r) = total.unwrap_or((0, 0));
                total = Some((a + added, r + removed));
            }
        }
        total.map(|(added, removed)| Diff {
            added,
            removed,
            estimated: false,
        })
    }

    /// 一次调用要人确认。
    pub(super) fn approval_requested(&mut self, requested: &ApprovalRequested) {
        let Some(id) = self.calls.get(&requested.call_id).cloned() else {
            return;
        };
        let approval = Approval {
            access: requested.access.clone(),
            rule: requested.rule.clone(),
            detail: requested.detail.clone(),
            decision: None,
            reason: None,
            by: None,
            nested: requested.nested,
        };
        self.touch(&id, |entry| {
            if let Body::Tool(tool) = &mut entry.body {
                tool.approval = Some(approval);
            }
        });
    }

    /// 确认有了决定。
    pub(super) fn approval_decided(&mut self, event: &Event, decided: &ApprovalDecided) {
        let Some(id) = self.calls.get(&decided.call_id).cloned() else {
            return;
        };
        self.touch(&id, |entry| {
            if let Body::Tool(tool) = &mut entry.body
                && let Some(approval) = tool.approval.as_mut()
            {
                approval.decision = Some(decided.decision.clone());
                approval.reason.clone_from(&decided.reason);
                approval.by = Some(event.by.clone());
            }
        });
    }

    /// 问了一组题：记着等答，那一步当场带上问的什么（施工 9-8 十四补，终端的抽屉要）。
    pub(super) fn asked(&mut self, asked: &QuestionAsked) {
        self.questions
            .insert(asked.call_id, asked.questions.clone());
        let Some(id) = self.calls.get(&asked.call_id).cloned() else {
            return;
        };
        self.touch(&id, |entry| {
            if let Body::Tool(tool) = &mut entry.body {
                tool.questions.clone_from(&asked.questions);
            }
        });
    }

    /// 一组题答了：另起一条旁白，带着问的那一组。
    pub(super) fn answered(&mut self, event: &Event, answered: &QuestionAnswered) {
        let questions = self.questions.remove(&answered.call_id).unwrap_or_default();
        self.notice(
            EntryId::event(event.seq),
            event.at,
            Notice::Answered(Answered {
                questions,
                answers: answered.answers.clone(),
                by: event.by.clone(),
            }),
        );
    }
}

/// 参数原文里的 `command`：后台命令派出时的那一句。
fn command_of(args: &str) -> Option<String> {
    serde_json::from_str::<Value>(args)
        .ok()
        .and_then(|a| a.get("command").and_then(Value::as_str).map(str::to_string))
}

/// 条目里的结果原文最多几行、几个字节（施工 9-8 十四补）：一页约 1 MiB，一步 8 KiB 放得下上百步。
const OUTPUT_LINES: usize = 100;
const OUTPUT_BYTES: usize = 8 * 1024;

/// 结果原文的前面一段：文字块照先后、块之间空一行（同 `view.detail` 的 `output`），最多 [`OUTPUT_LINES`] 行、
/// [`OUTPUT_BYTES`] 字节，先到的算，切在字的边界上；后面只剩空白的不算切。交回那一段和切没切，没有字的是 `None`。
fn output_head(blocks: &[Content]) -> (Option<String>, bool) {
    let whole = blocks
        .iter()
        .filter_map(|block| match block {
            Content::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect::<Vec<&str>>()
        .join("\n\n");
    if whole.is_empty() {
        return (None, false);
    }
    let mut end = whole
        .match_indices('\n')
        .nth(OUTPUT_LINES - 1)
        .map_or(whole.len(), |(at, _)| at);
    if end > OUTPUT_BYTES {
        end = OUTPUT_BYTES;
        while !whole.is_char_boundary(end) {
            end -= 1;
        }
    }
    if whole[end..].trim_end().is_empty() {
        return (Some(whole), false);
    }
    (Some(whole[..end].to_string()), true)
}
