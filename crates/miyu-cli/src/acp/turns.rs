//! 说一句、打断、斜杠命令（设计 73 第五节第 3、7 条，`internal/blueprint/cli/acp.md`「说一句」「斜杠命令」）：`session/prompt`
//! 照 `session.send` 说（附件先传），这一轮结束了才回（`view.rs`）；`session/cancel` 照 `session.interrupt` 打断，还在排队的
//! 那一句照 `session.withdraw` 退回；`/` 开头、是 Miyu 的命令的照 `command.run` 跑，`/compact` 是头自己的命令，照
//! `session.compact` 压。

use serde_json::{Value, json};

use super::editor::{self, chunk};
use super::prompt::{self, answered};
use super::server::Server;
use super::session::{Prompt, Waiting};
use crate::language::Language;

/// 头自己的命令：手动压缩（`command.catalog` 里没有它，同终端界面自己加进菜单）。
const COMPACT: &str = "compact";

impl Server {
    /// `session/prompt`。一个会话同时只等一句（规范：上一句回了才发下一句）。
    pub(crate) async fn prompt(&mut self, id: &Value, params: &Value) {
        let session = params["sessionId"].as_str().unwrap_or_default().to_string();
        let Some(open) = self.sessions.get(&session) else {
            let message = self.language.acp_no_session();
            return self.editor.unknown(id, &session, message).await;
        };
        if open.prompt.is_some() {
            let message = self.language.acp_busy();
            return self
                .editor
                .error(id, editor::INVALID_REQUEST, message, None)
                .await;
        }
        let Ok(said) = prompt::read(&params["prompt"]) else {
            let message = self.language.acp_bad_content();
            return self
                .editor
                .error(id, editor::INVALID_PARAMS, message, None)
                .await;
        };
        if said.text.trim().is_empty() && said.files.is_empty() {
            let message = self.language.acp_empty();
            return self
                .editor
                .error(id, editor::INVALID_PARAMS, message, None)
                .await;
        }
        if said.files.is_empty() && self.slash(id, &session, &said.text).await {
            return;
        }
        let attachments = match prompt::attach(&mut self.rpc, &said.files).await {
            Ok(attachments) => attachments,
            Err(error) => {
                let message = self.refused(&error);
                return self
                    .editor
                    .error(id, editor::INTERNAL, &message, None)
                    .await;
            }
        };
        let mut send = json!({"session": session, "text": said.text});
        if !attachments.is_empty() {
            send["attachments"] = json!(attachments);
        }
        let seq = match answered(self.rpc.call("session.send", send).await) {
            Ok(sent) => sent["events"][0].clone(),
            Err(error) => {
                let message = self.refused(&error);
                return self
                    .editor
                    .error(id, editor::INTERNAL, &message, None)
                    .await;
            }
        };
        let message = format!("m{seq}");
        if let Some(open) = self.sessions.get_mut(&session) {
            open.own.insert(message.clone());
            open.prompt = Some(Prompt {
                id: id.clone(),
                waiting: Waiting::Message(message),
                cancelled: false,
            });
        }
    }

    /// `/` 开头的一句：是 Miyu 的命令（或者头自己的 `/compact`）就照命令跑，交回真；不是的交回假，照平常的话说。
    async fn slash(&mut self, id: &Value, session: &str, text: &str) -> bool {
        let line = text.trim();
        let Some(rest) = line.strip_prefix('/') else {
            return false;
        };
        let (name, words) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        let known = self.sessions.get(session).is_some_and(|open| {
            open.commands.iter().any(|command| {
                command["name"] == name
                    || command["aliases"]
                        .as_array()
                        .is_some_and(|aliases| aliases.iter().any(|alias| alias == name))
            })
        });
        let (method, params) = match (name == COMPACT, known) {
            (true, false) => {
                let mut params = json!({"session": session});
                if !words.trim().is_empty() {
                    params["instructions"] = json!(words.trim());
                }
                ("session.compact", params)
            }
            (_, true) => {
                let cwd = self.sessions.get(session).map(|open| open.cwd.clone());
                (
                    "command.run",
                    json!({"session": session, "text": line, "cwd": cwd}),
                )
            }
            (false, false) => return false,
        };
        let ran = answered(self.rpc.call(method, params).await);
        let result = match ran {
            Ok(result) => result,
            // 命令被拒（没有能压的、有回合在进行……）：照核心的原话说给人，这一句照常结束。
            Err(error) => {
                let said = self.refused(&error);
                self.settle().await;
                self.editor
                    .update(session, chunk("agent_message_chunk", &said))
                    .await;
                self.reply(id, super::session::Stop::Reason("end_turn"))
                    .await;
                return true;
            }
        };
        if let Some(said) = result["said"].as_str().filter(|said| !said.is_empty()) {
            self.editor
                .update(session, chunk("agent_message_chunk", said))
                .await;
        }
        let events: Vec<Value> = result["events"].as_array().cloned().unwrap_or_default();
        // 手动压缩回的是它开的那一轮，压完才结束；命令回的是它追加的几条事件。
        let waiting = match (method, events.first()) {
            ("session.compact", Some(turn)) => Waiting::Turn(turn.clone()),
            _ => Waiting::Command(events.clone()),
        };
        if let Some(open) = self.sessions.get_mut(session) {
            for seq in &events {
                open.own.insert(format!("m{seq}"));
            }
            open.prompt = Some(Prompt {
                id: id.clone(),
                waiting,
                cancelled: false,
            });
        }
        // 命令引出的推送比回应先到，办完了还在等命令本身的（没开一轮、没说一句）就是做完了。
        self.settle().await;
        let done = self.sessions.get_mut(session).and_then(|open| {
            match open.prompt.as_ref().map(|prompt| &prompt.waiting) {
                Some(Waiting::Command(_)) => open.prompt.take(),
                _ => None,
            }
        });
        if let Some(prompt) = done {
            self.reply(&prompt.id, super::session::Stop::Reason("end_turn"))
                .await;
        }
        true
    }

    /// `session/cancel`：在等的那一句还排着的退回，回 `cancelled`；已经开了一轮的打断，这一轮结束时回 `cancelled`。
    pub(crate) async fn cancel(&mut self, params: &Value) {
        let session = params["sessionId"].as_str().unwrap_or_default().to_string();
        let Some(prompt) = self
            .sessions
            .get_mut(&session)
            .and_then(|open| open.prompt.as_mut())
        else {
            return;
        };
        prompt.cancelled = true;
        let queued = match &prompt.waiting {
            Waiting::Message(message) => message.strip_prefix('m').map(str::to_string),
            _ => None,
        };
        if let Some(seq) = queued.and_then(|seq| seq.parse::<u64>().ok()) {
            let params = json!({"session": session, "message": seq});
            if answered(self.rpc.call("session.withdraw", params).await).is_ok() {
                let taken = self
                    .sessions
                    .get_mut(&session)
                    .and_then(|open| open.prompt.take());
                if let Some(prompt) = taken {
                    self.reply(&prompt.id, super::session::Stop::Reason("cancelled"))
                        .await;
                }
                return;
            }
        }
        let params = json!({"session": session, "queued": "return"});
        let _sent = self.rpc.send("session.interrupt", params).await;
    }
}

/// 告诉编辑器这里认哪几条斜杠命令（`available_commands_update`）：Miyu 的命令表照 `command.catalog`（打开整页的不列，编辑器
/// 里打不开），加上头自己的 `/compact`。
pub(crate) fn advertised(language: Language, catalog: &[Value]) -> Value {
    let mut commands: Vec<Value> = catalog
        .iter()
        .filter(|command| command["opens"].is_null())
        .filter_map(|command| {
            let name = command["name"].as_str()?;
            let mut item =
                json!({"name": name, "description": command["summary"].as_str().unwrap_or(name)});
            if let Some(hint) = command["argument"].as_str().filter(|hint| !hint.is_empty()) {
                item["input"] = json!({"hint": hint});
            }
            Some(item)
        })
        .collect();
    if !commands.iter().any(|command| command["name"] == COMPACT) {
        commands.push(json!({
            "name": COMPACT,
            "description": language.acp_compact(),
            "input": {"hint": language.acp_compact_hint()},
        }));
    }
    json!({"sessionUpdate": "available_commands_update", "availableCommands": commands})
}
