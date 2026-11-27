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

use super::{Job, Projector, output_head};
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
            // 供应商侧工具（施工 HT-3）照对上的那件核心工具算，对不上的算别的工具；它的参数是供应商的写法，命令的短标题、
            // 留言的对象不照它取。
            if tool.hosted {
                let core = texts.kinds.hosted_as(&tool.name).map(|h| h.core.as_str());
                return Counted::Tool {
                    kind: core.and_then(|core| texts.kinds.of(core)),
                    failed: tool.state.failed(),
                    diff: (0, 0),
                    title: None,
                    to_session: false,
                    package: None,
                    sent: 0,
                    names: (
                        tool.title.name.clone(),
                        core.and_then(|core| texts.english.face(core).map(|face| face.name))
                            .unwrap_or_else(|| tool.name.clone()),
                    ),
                };
            }
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
        let standard_args = self.standard(name, &args, state);
        let parsed = estimate::parsed(standard_args.as_deref().unwrap_or(&args));
        let tool = Tool {
            group: group.clone(),
            call,
            name: name.to_string(),
            hosted: false,
            title: title::of(name, &parsed, state, None, &BTreeMap::new(), &self.texts),
            state,
            diff: self.estimate(name, &parsed, state),
            args,
            standard_args,
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
        let mut ready = None;
        if let Some(Body::Tool(tool)) = self.get(id).map(|e| &e.body)
            && tool.state == ToolState::Preparing
        {
            let standard = self.standard(&tool.name, &tool.args, ToolState::Running);
            let parsed = estimate::parsed(standard.as_deref().unwrap_or(&tool.args));
            let estimate = self.estimate(&tool.name, &parsed, ToolState::Running);
            ready = Some((
                estimate,
                self.to_title(&tool.name, &parsed),
                standard,
                parsed,
            ));
        }
        let Some((diff, to_title, standard, parsed)) = ready else {
            return;
        };
        self.touch(id, |entry| {
            if let Body::Tool(tool) = &mut entry.body {
                tool.standard_args = standard;
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
        let needs = match self.get(id).map(|e| &e.body) {
            Some(Body::Tool(tool)) if tool.call != Some(call) || tool.args != args => {
                Some(self.standard(&tool.name, args, tool.state))
            }
            _ => None,
        };
        if let Some(standard) = needs {
            let args = args.to_string();
            self.touch(id, |entry| {
                if let Body::Tool(tool) = &mut entry.body {
                    tool.call = Some(call);
                    tool.args = args;
                    tool.standard_args = standard;
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

    /// 编辑、写入的参数不是标准写法的：规整成标准写法的原文（施工 ED-3）；别的工具、还在写参数的、本来就是的没有。
    fn standard(&self, name: &str, args: &str, state: ToolState) -> Option<String> {
        (state != ToolState::Preparing && self.texts.kinds.of(name) == Some(ToolKind::Edit))
            .then(|| estimate::standard(args))
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
                let parsed = estimate::parsed(tool.standard_args.as_deref().unwrap_or(&tool.args));
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
        // 驱动问的画在它那一块供应商侧工具上（施工 ACP-8，`hosted.rs`）。
        if let Some(block) = requested.driver {
            return self.driver_asked(requested, block);
        }
        let Some(id) = self.calls.get(&requested.call_id).cloned() else {
            return;
        };
        let approval = super::hosted::approval_of(requested);
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
        // 驱动问的、那一步还没开条目的（读回日志时，施工 ACP-8）：记在手里的那一份上。
        if let Some(approval) = self.held.get_mut(&id).map(|(_, approval)| approval) {
            decide(approval, event, decided);
        }
        self.touch(&id, |entry| {
            if let Body::Tool(tool) = &mut entry.body
                && let Some(approval) = tool.approval.as_mut()
            {
                decide(approval, event, decided);
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

/// 确认有了决定：照 `decided` 写上怎么定的、为什么、谁定的。
fn decide(approval: &mut Approval, event: &Event, decided: &ApprovalDecided) {
    approval.decision = Some(decided.decision.clone());
    approval.reason.clone_from(&decided.reason);
    approval.by = Some(event.by.clone());
}

/// 参数原文里的 `command`：后台命令派出时的那一句。
fn command_of(args: &str) -> Option<String> {
    serde_json::from_str::<Value>(args)
        .ok()
        .and_then(|a| a.get("command").and_then(Value::as_str).map(str::to_string))
}
