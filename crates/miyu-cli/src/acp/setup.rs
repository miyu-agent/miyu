//! 握手和会话（设计 73 第五节第 1、2、5 条，`internal/blueprint/cli/acp.md`「握手」「会话」）：`initialize` 回能力；
//! `session/new` 造一个 Miyu 会话（工作目录是编辑器给的，权限照 Miyu 的默认）；`session/load` 订阅视图流、把历史重放成
//! `session/update`；`session/resume` 只接上；`session/list` 照工作目录列；`session/close` 停下这边的订阅；切模式、换配置项。
//!
//! 编辑器给的 `mcpServers` 不收、`mcpCapabilities` 不声明：核心的 MCP 服务一个核心一份、所有会话共用，没有「只对一个会话」的
//! 接法（设计 73 第五节第 6 条、第八节）。

use std::path::Path;

use serde_json::{Value, json};

use super::editor;
use super::options;
use super::prompt::answered;
use super::server::Server;
use super::session::Session;
use super::view::{Action, Pace};

/// 照的规范版本。编辑器报得更高的也回它（规范：不支持对方的版本就回自己支持的最高版本）。
const VERSION: u64 = 1;

/// 往前翻历史时一页最多几轮（`view.page` 的上限）。
const PAGE_TURNS: u64 = 50;

impl Server {
    /// `initialize`：回能力。`authMethods` 空：连的是本机核心、用本机令牌，不要编辑器登录。
    pub(crate) async fn initialize(&mut self, id: &Value, params: &Value) {
        let forms = &params["clientCapabilities"]["elicitation"]["form"];
        self.client.forms = forms.is_object();
        self.initialized = true;
        let result = json!({
            "protocolVersion": VERSION,
            "agentCapabilities": {
                "loadSession": true,
                "promptCapabilities": {"image": true, "audio": false, "embeddedContext": true},
                "sessionCapabilities": {"list": {}, "resume": {}, "close": {}},
            },
            "agentInfo": {"name": "miyu", "title": "Miyu", "version": env!("CARGO_PKG_VERSION")},
            "authMethods": [],
        });
        self.editor.result(id, result).await;
    }

    /// `session/new`：工作目录要是绝对路径；照 Miyu 的默认造会话，接上它的视图流。
    pub(crate) async fn new_session(&mut self, id: &Value, params: &Value) {
        let Some(cwd) = absolute(params) else {
            let message = self.language.acp_relative_cwd();
            return self
                .editor
                .error(id, editor::INVALID_PARAMS, message, None)
                .await;
        };
        ignore_servers(params);
        let created = answered(self.rpc.call("session.create", json!({"cwd": cwd})).await);
        let session = match created {
            Ok(created) => created["session"].as_str().unwrap_or_default().to_string(),
            Err(error) => {
                let message = self.refused(&error);
                return self
                    .editor
                    .error(id, editor::INTERNAL, &message, None)
                    .await;
            }
        };
        self.attach(id, &session, &cwd, Pace::Quiet, true).await;
    }

    /// `session/load`（`pace` 是重放）、`session/resume`（只记下）：接上编辑器给的那个会话。
    pub(crate) async fn open(&mut self, id: &Value, params: &Value, pace: Pace) {
        let session = params["sessionId"].as_str().unwrap_or_default().to_string();
        let Some(cwd) = absolute(params) else {
            let message = self.language.acp_relative_cwd();
            return self
                .editor
                .error(id, editor::INVALID_PARAMS, message, None)
                .await;
        };
        ignore_servers(params);
        self.attach(id, &session, &cwd, pace, false).await;
    }

    /// 订阅会话 `session` 的视图流，照 `pace` 过一遍已有的条目（重放的往前翻到头），记下会话，回编辑器（新开的带
    /// `sessionId`），再报斜杠命令、问在等的确认。
    async fn attach(&mut self, id: &Value, session: &str, cwd: &str, pace: Pace, created: bool) {
        let params = json!({"session": session, "stream": "view"});
        let subscribed = answered(self.rpc.call("subscribe", params).await);
        let result = match subscribed {
            Ok(result) => result,
            Err(error) if error["data"]["reason"] == "session_not_found" => {
                let message = self.language.acp_no_session();
                return self.editor.unknown(id, session, message).await;
            }
            Err(error) => {
                let message = self.refused(&error);
                return self
                    .editor
                    .error(id, editor::INTERNAL, &message, None)
                    .await;
            }
        };
        let entries = match pace {
            Pace::Replay => self.history(session, &result).await,
            _ => result["entries"].as_array().cloned().unwrap_or_default(),
        };
        let mut opened = Session::new(cwd);
        let mut actions = Vec::new();
        for entry in &entries {
            actions.extend(opened.entry(entry, pace));
        }
        actions.extend(opened.status(&result["status"]));
        opened.commands = self.catalog(session).await;
        let commands = opened.commands.clone();
        // 能选的模型每次都重问（施工 ACP-9）：新装的包带的供应商，列表可能是上一次以后才拉到的。变了的，别的会话也报一次。
        self.refresh_choices().await;
        self.sessions.insert(session.to_string(), opened);
        // 回应里带着整份配置项，变了的那一下不另报；在等人的回了编辑器再问。
        let (now, later): (Vec<Action>, Vec<Action>) = actions
            .into_iter()
            .filter(|action| *action != Action::Config)
            .partition(|action| matches!(action, Action::Update(_)));
        self.act(session, now).await;
        let mut reply = self.described(session);
        if created {
            reply["sessionId"] = json!(session);
        }
        self.editor.result(id, reply).await;
        let update = super::turns::advertised(self.language, &commands);
        self.editor.update(session, update).await;
        self.act(session, later).await;
    }

    /// 重放要的整段历史：订阅回应里的最新一页，再照 `view.page` 往前一页页翻到头，照先后接起来。翻不动的（核心拒了）到那里为止。
    async fn history(&mut self, session: &str, subscribed: &Value) -> Vec<Value> {
        let mut pages = vec![
            subscribed["entries"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
        ];
        let (mut more, mut first) = (subscribed["more"] == true, subscribed["first"].clone());
        while more && !first.is_null() {
            let params =
                json!({"session": session, "before": first, "turns": PAGE_TURNS, "view": true});
            let Ok(page) = answered(self.rpc.call("view.page", params).await) else {
                break;
            };
            pages.push(page["entries"].as_array().cloned().unwrap_or_default());
            more = page["more"] == true;
            first = page["first"].clone();
        }
        pages.into_iter().rev().flatten().collect()
    }

    /// 这个会话认的斜杠命令（`command.catalog`）：名字连同别名；问不到的是空的。
    async fn catalog(&mut self, session: &str) -> Vec<Value> {
        let params = json!({"session": session});
        answered(self.rpc.call("command.catalog", params).await)
            .ok()
            .and_then(|catalog| catalog["commands"].as_array().cloned())
            .unwrap_or_default()
    }

    /// 回 `session/new`、`load`、`resume` 时带的模式和配置项。
    fn described(&self, session: &str) -> Value {
        let choices = self.choices.clone().unwrap_or_default();
        let current = self
            .sessions
            .get(session)
            .map(|session| session.current.clone())
            .unwrap_or_default();
        json!({
            "modes": options::modes(self.language, &current),
            "configOptions": options::options(self.language, &choices, &current),
        })
    }

    /// `session/list`：Miyu 的主会话（不列子代理、整理记忆的会话），工作目录照编辑器给的过滤，从新到旧，不分页。
    pub(crate) async fn list(&mut self, id: &Value, params: &Value) {
        let listed = answered(self.rpc.call("session.list", json!({"dream": false})).await);
        let listed = match listed {
            Ok(listed) => listed,
            Err(error) => {
                let message = self.refused(&error);
                return self
                    .editor
                    .error(id, editor::INTERNAL, &message, None)
                    .await;
            }
        };
        let wanted = params["cwd"].as_str().map(same_place);
        let sessions: Vec<Value> = listed["sessions"]
            .as_array()
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .filter(|item| item["parent"].is_null())
            .filter_map(|item| {
                let cwd = expand(item["cwd"].as_str()?, self.home.as_deref());
                if wanted
                    .as_ref()
                    .is_some_and(|wanted| *wanted != same_place(&cwd))
                {
                    return None;
                }
                let mut info = json!({"sessionId": item["session"], "cwd": cwd});
                if let Some(title) = item["title"].as_str().or(item["preview"].as_str()) {
                    info["title"] = json!(title);
                }
                if let Some(at) = item["last_active"].as_str() {
                    info["updatedAt"] = json!(at);
                }
                Some(info)
            })
            .collect();
        self.editor.result(id, json!({"sessions": sessions})).await;
    }

    /// `session/close`：这边停下订阅、忘掉会话；有一句在等的先照 `session/cancel` 打断，回 `cancelled`。会话本身留在 Miyu 里。
    /// 没有这个会话的照样回 `{}`（规范：关会话是幂等的）。
    pub(crate) async fn close(&mut self, id: &Value, params: &Value) {
        let session = params["sessionId"].as_str().unwrap_or_default().to_string();
        if let Some(mut closed) = self.sessions.remove(&session) {
            if let Some(prompt) = closed.prompt.take() {
                let interrupt = json!({"session": session, "queued": "return"});
                let _sent = self.rpc.send("session.interrupt", interrupt).await;
                self.editor
                    .result(&prompt.id, json!({"stopReason": "cancelled"}))
                    .await;
            }
            self.asks.retain(|_, ask| ask.session() != session);
            let params = json!({"session": session, "stream": "view"});
            let _sent = self.rpc.send("unsubscribe", params).await;
        }
        self.editor.result(id, json!({})).await;
    }

    /// `session/set_mode`（`full` 是假）、`session/set_config_option`（`full` 是真，回整份配置项）：照项换，认不得的项、值回
    /// `-32602`。
    pub(crate) async fn configure(
        &mut self,
        id: &Value,
        params: &Value,
        option: &str,
        value: &str,
        full: bool,
    ) {
        let session = params["sessionId"].as_str().unwrap_or_default().to_string();
        let Some(current) = self.sessions.get(&session).map(|s| s.current.clone()) else {
            let message = self.language.acp_no_session();
            return self.editor.unknown(id, &session, message).await;
        };
        let choices = self.choices.clone().unwrap_or_default();
        let Some((method, call)) = options::request(&session, &choices, &current, option, value)
        else {
            let message = self.language.acp_bad_option();
            return self
                .editor
                .error(id, editor::INVALID_PARAMS, message, None)
                .await;
        };
        if let Err(error) = answered(self.rpc.call(method, call).await) {
            let message = self.refused(&error);
            return self
                .editor
                .error(id, editor::INTERNAL, &message, None)
                .await;
        }
        let after = options::applied(&current, option, value);
        let reply = match full {
            true => json!({"configOptions": options::options(self.language, &choices, &after)}),
            false => json!({}),
        };
        if let Some(open) = self.sessions.get_mut(&session) {
            open.current = after;
        }
        self.editor.result(id, reply).await;
    }
}

/// 编辑器给的工作目录：要是绝对路径，去掉 `.`；不是的交回 `None`。
fn absolute(params: &Value) -> Option<String> {
    let cwd = Path::new(params["cwd"].as_str()?);
    cwd.is_absolute()
        .then(|| cwd.components().collect::<std::path::PathBuf>())
        .map(|path| path.to_string_lossy().into_owned())
}

/// 编辑器给的 MCP 服务这一版不收（见模块开头），运行日志里记一行有几个。
fn ignore_servers(params: &Value) {
    let count = params["mcpServers"].as_array().map_or(0, Vec::len);
    if count > 0 {
        tracing::info!(target: "miyu::acp", count, "editor mcp servers ignored");
    }
}

/// 会话列表里的工作目录（人看到的写法，`~/…`）写成绝对路径：照核心所在机器的家目录 `home`；读不出家目录的照原样。
fn expand(cwd: &str, home: Option<&str>) -> String {
    match (cwd, home) {
        ("~", Some(home)) => home.to_string(),
        (_, Some(home)) if cwd.starts_with("~/") || cwd.starts_with("~\\") => Path::new(home)
            .join(&cwd[2..])
            .to_string_lossy()
            .into_owned(),
        _ => cwd.to_string(),
    }
}

/// 比两个工作目录是不是同一个用的写法：解开链接的；解不开的（目录没了）照原样去掉末尾的分隔符。
fn same_place(path: &str) -> std::path::PathBuf {
    let path = Path::new(path);
    path.canonicalize()
        .unwrap_or_else(|_| path.components().collect())
}

#[cfg(test)]
mod tests;
