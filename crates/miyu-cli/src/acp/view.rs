//! 视图流翻成 `session/update`（设计 73 第五节第 3 条，`internal/blueprint/cli/acp.md`「一轮怎么翻」）：正文
//! `agent_message_chunk`、思考 `agent_thought_chunk`、工具步骤 `tool_call` / `tool_call_update`，别处来的话
//! `user_message_chunk`；一轮的收尾回等着的 `session/prompt`。会话状态那一半在 `status.rs`。
//!
//! 纯逻辑：只记报到哪了、交回要做的（[`Action`]），写出去、要 `view.detail` 的由 `server.rs` 办。

use serde_json::Value;

use miyu_view::{Body, Entry, Tool, ToolState};

use super::editor::chunk;
use super::session::{Known, Session, Stop, Waiting};
use super::tool;

/// 一条推送引出的要做的事。
#[derive(Debug, PartialEq)]
pub(crate) enum Action {
    /// 报一条 `session/update`。
    Update(Value),
    /// 一步改了文件、刚有结果：先照调用编号 `call` 要 `view.detail`，把对比补进这一条再报。
    Detailed {
        /// 要报的 `tool_call_update`（或 `tool_call`）。
        update: Value,
        /// 调用编号。
        call: String,
    },
    /// 回等着的 `session/prompt`：编辑器那条请求的编号，怎么回。
    Answer {
        /// 请求的编号。
        id: Value,
        /// 怎么回。
        stop: Stop,
    },
    /// 会话在等人：确认、提问，照先后。
    Waiting(Vec<miyu_view::Waiting>),
    /// 一轮不是人说话开的（后台任务报回来叫醒的、到点的）：先报一句开头（`user_message_chunk`），编辑器才另起一条消息。
    Woken {
        /// 哪一轮（写成字）：开头那一句的 `messageId` 照它。
        turn: String,
        /// 叫醒它的后台任务：标题、怎么结束的；不知道的没有。
        job: Option<(String, String)>,
    },
    /// 权限、模型、思考档位变了：报 `current_mode_update`、`config_option_update`。
    Config,
}

/// 一条条目怎么报。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Pace {
    /// 正在发生：编辑器自己发的话不报回去，结束了回等着的请求，改了文件的要对比。
    Live,
    /// 重放历史（`session/load`）：人说的话都报，不回请求，不要对比。
    Replay,
    /// 只记下、不报（`session/resume`、刚开的会话）：之后来的变化照它算。
    Quiet,
}

impl Session {
    /// 视图流的一条推送。`view.hidden`、`view.remove` 不报：ACP 没有收回报过的内容的写法。
    pub(crate) fn push(&mut self, method: &str, params: &Value) -> Vec<Action> {
        match method {
            "view.add" | "view.update" => self.entry(&params["entry"], Pace::Live),
            "view.append" => self.append(&params["id"], &params["text"]),
            "view.status" => self.status(&params["status"]),
            _ => Vec::new(),
        }
    }

    /// 一条条目，照 `pace` 报。认不得的（以后的版本加的种类）不报。
    pub(crate) fn entry(&mut self, raw: &Value, pace: Pace) -> Vec<Action> {
        // 后台任务结束的旁白：记下它，叫醒的那一轮的开头照它说（只看这几格，旁白别的格变了也认得）。
        if raw["kind"] == "notice" && raw["what"] == "job" && raw["hidden"] != true {
            let title = raw["title"].as_str().unwrap_or_default().to_string();
            let mark = raw["mark"].as_str().unwrap_or_default().to_string();
            self.finished_job = Some((title, mark));
        }
        let Ok(entry) = serde_json::from_value::<Entry>(raw.clone()) else {
            return Vec::new();
        };
        if entry.hidden {
            return Vec::new();
        }
        let id = entry.id.to_string();
        let turn = raw.get("turn").filter(|turn| !turn.is_null());
        match entry.body {
            Body::User(user) => self.user(&id, &user, turn, pace),
            Body::Reply(reply) => {
                let mut out = self.introduce(turn, pace);
                out.extend(self.text(&id, "agent_message_chunk", &reply.text, pace));
                out
            }
            Body::Thought(thought) => {
                let mut out = self.introduce(turn, pace);
                out.extend(self.text(&id, "agent_thought_chunk", &thought.text, pace));
                out
            }
            Body::Tool(step) => {
                let mut out = self.introduce(turn, pace);
                out.extend(self.tool(&id, step, pace));
                out
            }
            Body::End(end) => {
                self.known.insert(id, Known::Other);
                match (pace, self.ended(&end, turn)) {
                    (Pace::Live, Some((id, stop))) => vec![Action::Answer { id, stop }],
                    _ => Vec::new(),
                }
            }
            _ => {
                self.known.insert(id, Known::Other);
                Vec::new()
            }
        }
    }

    /// 这一轮在编辑器里还没有开头的，先交回一句开头（[`Action::Woken`]）：不是人说话开的那几轮。编辑器在等的那一句还没
    /// 听到的、等的就是这一轮的，当这一轮是它开的。只记下的只记，不报。
    fn introduce(&mut self, turn: Option<&Value>, pace: Pace) -> Vec<Action> {
        let Some(turn) = turn else {
            return Vec::new();
        };
        let key = turn.to_string();
        if !self.introduced.insert(key.clone()) || pace == Pace::Quiet {
            return Vec::new();
        }
        let ours = match self.prompt.as_ref().map(|prompt| &prompt.waiting) {
            Some(Waiting::Message(_) | Waiting::Command(_)) => true,
            Some(Waiting::Turn(waited)) => waited == turn,
            None => false,
        };
        match ours {
            true => Vec::new(),
            false => vec![Action::Woken {
                turn: key,
                job: self.finished_job.take(),
            }],
        }
    }

    /// 人说的话：编辑器自己发的不报（只看它是哪一轮），别处来的报一次（终端、网页里接着说的）；退回的不报。它所属的那一轮
    /// 就有了开头。
    fn user(
        &mut self,
        id: &str,
        user: &miyu_view::User,
        turn: Option<&Value>,
        pace: Pace,
    ) -> Vec<Action> {
        if let Some(turn) = turn {
            self.introduced.insert(turn.to_string());
            self.finished_job = None;
        }
        if self.own.contains(id) {
            self.heard(id, turn);
            return Vec::new();
        }
        let first = !self.known.contains_key(id);
        self.known.insert(id.to_string(), Known::Other);
        match first && pace != Pace::Quiet && !user.withdrawn && !user.text.is_empty() {
            true => vec![Action::Update(chunk("user_message_chunk", &user.text, id))],
            false => Vec::new(),
        }
    }

    /// 正文、思考：整份的字，报过的以后的那一截。前面的对不上的（重写过）不再报，照新的记。
    fn text(&mut self, id: &str, kind: &'static str, text: &str, pace: Pace) -> Vec<Action> {
        let sent = match self.known.get(id) {
            Some(Known::Text { sent, .. }) => sent.clone(),
            _ => String::new(),
        };
        self.known.insert(
            id.to_string(),
            Known::Text {
                kind,
                sent: text.to_string(),
            },
        );
        match text.strip_prefix(sent.as_str()) {
            Some(more) if !more.is_empty() && pace != Pace::Quiet => {
                vec![Action::Update(chunk(kind, more, id))]
            }
            _ => Vec::new(),
        }
    }

    /// 正在写的字接上一截：只认报过的正文、思考（工具的参数不报）。
    fn append(&mut self, id: &Value, text: &Value) -> Vec<Action> {
        let (Some(id), Some(text)) = (id.as_str(), text.as_str()) else {
            return Vec::new();
        };
        let Some(Known::Text { kind, sent }) = self.known.get_mut(id) else {
            return Vec::new();
        };
        sent.push_str(text);
        vec![Action::Update(chunk(kind, text, id))]
    }

    /// 一步工具：第一次见的报 `tool_call`，见过的报变了的那几格。刚有结果、改了文件的，要对比（只在正在发生时）。
    /// 只记下的（[`Pace::Quiet`]）只记做完了的：还在跑的之后再来，照第一次见报，编辑器才认得这个编号。
    fn tool(&mut self, id: &str, step: Tool, pace: Pace) -> Vec<Action> {
        let before = match self.known.get(id) {
            Some(Known::Tool(before)) => Some(before.clone()),
            _ => None,
        };
        if pace == Pace::Quiet {
            if !step.state.busy() {
                self.known
                    .insert(id.to_string(), Known::Tool(Box::new(step)));
            }
            return Vec::new();
        }
        let update = match &before {
            Some(before) => tool::changed(id, before, &step, &self.cwd),
            None => Some(tool::opened(id, &step, &self.cwd)),
        };
        // 结果到了、或者改了的文件这一下才有（结果和文件不一定在同一条推送里）。
        let fresh = before
            .as_ref()
            .is_none_or(|before| before.state.busy() || before.files != step.files);
        let wants = pace == Pace::Live && fresh && changed_files(&step);
        let call = step.call.as_ref().map(ToString::to_string);
        self.known
            .insert(id.to_string(), Known::Tool(Box::new(step)));
        match (update, call) {
            (Some(update), Some(call)) if wants => vec![Action::Detailed { update, call }],
            (Some(update), _) => vec![Action::Update(update)],
            (None, _) => Vec::new(),
        }
    }

    /// 报过的一步工具的最后一份（问权限时带上标题、参数）。
    pub(crate) fn step(&self, id: &str) -> Option<&Tool> {
        match self.known.get(id) {
            Some(Known::Tool(tool)) => Some(tool),
            _ => None,
        }
    }
}

/// 做成了、改了（新建、改动）文件：要对比。扔进回收站的只报位置。
fn changed_files(step: &Tool) -> bool {
    use miyu_view::TouchKind;
    step.state == ToolState::Ok
        && step
            .files
            .iter()
            .any(|file| matches!(file.action, TouchKind::Created | TouchKind::Changed))
}

#[cfg(test)]
mod tests;
