//! `miyu acp` 的主循环（`internal/blueprint/cli/acp.md`「怎么走」）：一头是编辑器（标准输入读、标准输出写），一头是核心（一条
//! 连接，几个会话的视图流都订在它上面）。编辑器的请求照方法分到 `setup.rs`（握手、会话）、`turns.rs`（说一句、打断、斜杠
//! 命令）、`asking.rs`（编辑器回的确认、表单）；核心的推送照会话交给 `view.rs`、`status.rs` 翻，翻出来的事在这里办。

use std::collections::HashMap;

use serde_json::{Value, json};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite};

use miyu_ipc::Connection;

use super::asking::Ask;
use super::editor::{self, Editor};
use super::options::{self, Choices};
use super::session::{Session, Stop};
use super::tool;
use super::view::{Action, Pace};
use crate::exit;
use crate::language::Language;
use crate::rpc::Rpc;

/// 编辑器在握手时说它能做的、这边用得上的。
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Client {
    /// 能弹表单（`elicitation.form`）：提问走表单，补充的字也收得到。
    pub(crate) forms: bool,
}

/// 两头之间的那一个。
pub(crate) struct Server {
    /// 连着核心。
    pub(crate) rpc: Rpc,
    /// 对着编辑器。
    pub(crate) editor: Editor,
    /// 界面语言：握手以后照核心回的。
    pub(crate) language: Language,
    /// 核心所在机器的家目录（握手回的 `host.home`）：会话列表里的 `~/…` 照它写成绝对路径。
    pub(crate) home: Option<String>,
    /// 编辑器里开着的会话。
    pub(crate) sessions: HashMap<String, Session>,
    /// 问了编辑器、还没等到回答的：请求编号到问的什么。
    pub(crate) asks: HashMap<u64, Ask>,
    /// 编辑器握过手了。
    pub(crate) initialized: bool,
    /// 编辑器能做的。
    pub(crate) client: Client,
    /// 能选的模型：第一次开会话时问一次核心。
    pub(crate) choices: Option<Choices>,
}

/// 连上了核心的 `connection`：握手（本机令牌 `token`），之后一行行读编辑器的 `input`、往 `output` 写，直到编辑器关了管道
/// （退出码 0）或者核心断了（退出码 1）。`language` 是握手以前的界面语言。
pub async fn serve(
    connection: Connection,
    token: &str,
    input: impl AsyncBufRead + Unpin + Send,
    output: impl AsyncWrite + Unpin + Send + 'static,
    language: Language,
) -> u8 {
    let mut rpc = Rpc::new(connection, "acp");
    let hello = json!({
        "protocol": [1, 1],
        "head": {"kind": "acp", "version": env!("CARGO_PKG_VERSION")},
        "locale": language.locale(),
        "caps": {"input": true},
        "token": token,
    });
    let hello = match super::prompt::answered(rpc.call("hello", hello).await) {
        Ok(hello) => hello,
        Err(error) => {
            tracing::warn!(target: "miyu::acp", %error, "hello refused");
            return exit::ERROR;
        }
    };
    let mut server = Server {
        rpc,
        editor: Editor::new(Box::new(output)),
        language: crate::link::spoken(&hello, language),
        home: hello["host"]["home"].as_str().map(str::to_string),
        sessions: HashMap::new(),
        asks: HashMap::new(),
        initialized: false,
        client: Client::default(),
        choices: None,
    };
    let mut lines = input.lines();
    loop {
        tokio::select! {
            line = lines.next_line() => match line {
                Ok(Some(line)) => server.line(&line).await,
                _ => return exit::OK,
            },
            message = server.rpc.next() => match message {
                Some(message) => server.core(message).await,
                None => {
                    server.core_gone().await;
                    return exit::ERROR;
                }
            },
        }
    }
}

impl Server {
    /// 编辑器的一行。空行不管；读不懂的回 `-32700`。
    async fn line(&mut self, line: &str) {
        if line.trim().is_empty() {
            return;
        }
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            let message = self.language.acp_unreadable();
            return self.editor.error(&Value::Null, -32700, message, None).await;
        };
        let method = message.get("method").and_then(Value::as_str);
        let id = message.get("id").filter(|id| !id.is_null()).cloned();
        let params = message.get("params").cloned().unwrap_or(json!({}));
        match (method, id) {
            (Some(method), Some(id)) => self.request(method, &id, &params).await,
            (Some("session/cancel"), None) => self.cancel(&params).await,
            (Some(_), None) => {}
            (None, Some(id)) => self.answered(&id, &message).await,
            (None, None) => {}
        }
    }

    /// 编辑器的一条请求。握手以前只认 `initialize`。
    async fn request(&mut self, method: &str, id: &Value, params: &Value) {
        if !self.initialized && method != "initialize" {
            let message = self.language.acp_not_initialized();
            return self
                .editor
                .error(id, editor::INVALID_REQUEST, message, None)
                .await;
        }
        match method {
            "initialize" => self.initialize(id, params).await,
            "session/new" => self.new_session(id, params).await,
            "session/load" => self.open(id, params, Pace::Replay).await,
            "session/resume" => self.open(id, params, Pace::Quiet).await,
            "session/list" => self.list(id, params).await,
            "session/close" => self.close(id, params).await,
            "session/prompt" => self.prompt(id, params).await,
            "session/set_mode" => {
                let mode = params["modeId"].as_str().unwrap_or_default().to_string();
                self.configure(id, params, "mode", &mode, false).await;
            }
            "session/set_config_option" => {
                let option = params["configId"].as_str().unwrap_or_default().to_string();
                let value = params["value"].as_str().unwrap_or_default().to_string();
                self.configure(id, params, &option, &value, true).await;
            }
            _ => {
                let message = self.language.acp_no_method(method);
                self.editor
                    .error(id, editor::METHOD_NOT_FOUND, &message, None)
                    .await;
            }
        }
    }

    /// 核心的一条：推送照会话翻；回应（发了不等的那几条）不管。
    pub(crate) async fn core(&mut self, message: Value) {
        if message.get("id").is_some() {
            return;
        }
        let method = message["method"].as_str().unwrap_or_default();
        let params = &message["params"];
        let Some(id) = params["session"].as_str().map(str::to_string) else {
            return;
        };
        if method == "resync" {
            return self.resync(&id).await;
        }
        let Some(session) = self.sessions.get_mut(&id) else {
            return;
        };
        let actions = session.push(method, params);
        self.act(&id, actions).await;
    }

    /// 掉了队：重新订阅，回应里那一页条目照「正在发生」再过一遍（报过的只报变了的），会话状态也是。
    async fn resync(&mut self, id: &str) {
        if !self.sessions.contains_key(id) {
            return;
        }
        let params = json!({"session": id, "stream": "view"});
        let Ok(result) = super::prompt::answered(self.rpc.call("subscribe", params).await) else {
            return;
        };
        let Some(session) = self.sessions.get_mut(id) else {
            return;
        };
        let mut actions = Vec::new();
        for entry in result["entries"].as_array().map_or(&[][..], Vec::as_slice) {
            actions.extend(session.entry(entry, Pace::Live));
        }
        actions.extend(session.status(&result["status"]));
        self.act(id, actions).await;
    }

    /// 办翻出来的事。
    pub(crate) async fn act(&mut self, session: &str, actions: Vec<Action>) {
        for action in actions {
            match action {
                Action::Update(update) => self.editor.update(session, update).await,
                Action::Detailed { mut update, call } => {
                    let params = json!({"session": session, "call": call});
                    if let Ok(detail) =
                        super::prompt::answered(self.rpc.call("view.detail", params).await)
                    {
                        let diffs = tool::diffs(&detail);
                        if !diffs.is_empty() {
                            update["content"] = json!(diffs);
                        }
                    }
                    self.editor.update(session, update).await;
                }
                Action::Answer { id, stop } => self.reply(&id, stop).await,
                Action::Waiting(waiting) => self.ask(session, waiting).await,
                Action::Config => self.config_changed(session).await,
                Action::Woken { turn, job } => {
                    let text = self.language.acp_woken(
                        job.as_ref()
                            .map(|(title, mark)| (title.as_str(), mark.as_str())),
                    );
                    let message = format!("turn-{turn}");
                    let update = editor::chunk("user_message_chunk", &text, &message);
                    self.editor.update(session, update).await;
                }
            }
        }
    }

    /// 回一条 `session/prompt`。
    pub(crate) async fn reply(&mut self, id: &Value, stop: Stop) {
        match stop {
            Stop::Reason(reason) => {
                self.editor.result(id, json!({"stopReason": reason})).await;
            }
            Stop::Failed(message) => {
                self.editor
                    .error(id, editor::INTERNAL, &message, None)
                    .await;
            }
        }
    }

    /// 权限、模型、思考档位变了：报当前的模式和整份配置项。
    async fn config_changed(&mut self, id: &str) {
        let Some(session) = self.sessions.get(id) else {
            return;
        };
        let mode =
            json!({"sessionUpdate": "current_mode_update", "currentModeId": session.current.mode});
        let choices = self.choices.clone().unwrap_or_default();
        let configured = options::options(self.language, &choices, &session.current);
        let update = json!({"sessionUpdate": "config_option_update", "configOptions": configured});
        self.editor.update(id, mode).await;
        self.editor.update(id, update).await;
    }

    /// 核心断了：等着回的 `session/prompt` 都照出错回。
    async fn core_gone(&mut self) {
        let message = self.language.acp_disconnected().to_string();
        let waiting: Vec<Value> = self
            .sessions
            .values_mut()
            .filter_map(|session| session.prompt.take().map(|prompt| prompt.id))
            .collect();
        for id in waiting {
            self.editor
                .error(&id, editor::INTERNAL, &message, None)
                .await;
        }
    }

    /// 等回应时留下的推送都办完：斜杠命令引出的推送比命令的回应先到（`04-核心协议.md` 第六节第 2 条），办完了才知道它开没
    /// 开一轮。
    pub(crate) async fn settle(&mut self) {
        while let Some(message) = self.rpc.held() {
            self.core(message).await;
        }
    }

    /// 核心回的错误里给人看的那一句；断了的说断了。
    pub(crate) fn refused(&self, error: &Value) -> String {
        match error["message"].as_str() {
            Some(message) => message.to_string(),
            None => self.language.acp_disconnected().to_string(),
        }
    }
}
