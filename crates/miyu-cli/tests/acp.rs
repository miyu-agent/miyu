//! `miyu acp`（施工 ACP-6，`internal/blueprint/cli/acp.md`）：在进程里起一个核心（假模型照剧本回），测试自己当编辑器，在一对管子
//! 上直接写 JSON-RPC，看 `miyu acp` 回什么、报什么。

mod asking;
mod sessions;
mod turns;

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, Lines};
use tokio::task::JoinHandle;

use crate::support::{Home, within};
use miyu_cli::language::Language;
use miyu_session::testkit::{Play, Script};

/// 起一个核心：请求模型照 `plays`，工具是出厂的那一套。
pub fn home(plays: impl IntoIterator<Item = Play>) -> Home {
    Home::with_tools(
        Arc::new(Script::new(plays)),
        crate::support::shipped_tools(),
    )
}

/// 测试当的编辑器：一头写给 `miyu acp`，一头读它写的；`miyu acp` 在另一个任务里跑。
pub struct Editor {
    to_agent: DuplexStream,
    from_agent: Lines<BufReader<DuplexStream>>,
    next: u64,
    /// `miyu acp` 跑完交回的退出码。
    running: JoinHandle<u8>,
}

impl Editor {
    /// 连上 `home` 的核心，起一个 `miyu acp`，握好手（编辑器能做的照 `capabilities`）。交回握手的回应。
    pub async fn with(home: &Home, capabilities: Value) -> (Editor, Value) {
        let (connection, token) = miyu_ipc::connect(&home.root).await.expect("连得上");
        let (to_agent, agent_in) = tokio::io::duplex(1 << 20);
        let (agent_out, from_agent) = tokio::io::duplex(1 << 20);
        let running = tokio::spawn(async move {
            miyu_cli::acp_on(
                connection,
                &token,
                BufReader::new(agent_in),
                agent_out,
                Language::Chinese,
            )
            .await
        });
        let mut editor = Editor {
            to_agent,
            from_agent: BufReader::new(from_agent).lines(),
            next: 0,
            running,
        };
        let params = json!({"protocolVersion": 1, "clientCapabilities": capabilities,
            "clientInfo": {"name": "test", "version": "0"}});
        let (_, hello) = editor.call("initialize", params).await;
        (editor, hello)
    }

    /// 编辑器关了管道：等 `miyu acp` 退出，交回退出码。
    pub async fn close(self) -> u8 {
        drop(self.to_agent);
        within("miyu acp 退出", self.running).await.expect("跑完了")
    }

    /// 同 [`Editor::with`]，编辑器什么能力都不声明。
    pub async fn new(home: &Home) -> Editor {
        Editor::with(home, json!({})).await.0
    }

    /// 写一行。
    pub async fn write(&mut self, message: Value) {
        let line = format!("{message}\n");
        self.to_agent
            .write_all(line.as_bytes())
            .await
            .expect("写得进");
    }

    /// 发一条请求，交回编号。
    pub async fn send(&mut self, method: &str, params: Value) -> u64 {
        self.next += 1;
        let id = self.next;
        self.write(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .await;
        id
    }

    /// 发一条通知。
    pub async fn notify(&mut self, method: &str, params: Value) {
        self.write(json!({"jsonrpc": "2.0", "method": method, "params": params}))
            .await;
    }

    /// 回 `miyu acp` 问的一条。
    pub async fn reply(&mut self, id: &Value, result: Value) {
        self.write(json!({"jsonrpc": "2.0", "id": id, "result": result}))
            .await;
    }

    /// 读下一条，最多等六十秒。
    pub async fn read(&mut self) -> Value {
        let line = within("miyu acp 写下一条", self.from_agent.next_line())
            .await
            .expect("读得了")
            .expect("还开着");
        serde_json::from_str(&line).expect("一行 JSON")
    }

    /// 发一条请求，读到它的回应为止：交回中间读到的几条（通知、问编辑器的请求）和回应。
    pub async fn call(&mut self, method: &str, params: Value) -> (Vec<Value>, Value) {
        let id = self.send(method, params).await;
        self.until_reply(id).await
    }

    /// 读到编号是 `id` 的回应为止。
    pub async fn until_reply(&mut self, id: u64) -> (Vec<Value>, Value) {
        let mut seen = Vec::new();
        loop {
            let message = self.read().await;
            if message["id"] == json!(id) && message.get("method").is_none() {
                return (seen, message);
            }
            seen.push(message);
        }
    }

    /// 读到 `wanted` 认的那一条为止：交回中间读到的几条和它。
    pub async fn until(&mut self, wanted: impl Fn(&Value) -> bool) -> (Vec<Value>, Value) {
        let mut seen = Vec::new();
        loop {
            let message = self.read().await;
            if wanted(&message) {
                return (seen, message);
            }
            seen.push(message);
        }
    }

    /// 开一个新会话，工作目录 `cwd`：交回会话编号和回应；斜杠命令那一条读掉。
    pub async fn open(&mut self, cwd: &str) -> (String, Value) {
        let (_, reply) = self
            .call("session/new", json!({"cwd": cwd, "mcpServers": []}))
            .await;
        let session = reply["result"]["sessionId"]
            .as_str()
            .expect("开出来了")
            .to_string();
        self.until(|message| kind(message) == Some("available_commands_update"))
            .await;
        (session, reply)
    }

    /// 说一句（只有字）：交回中间读到的几条和回应。
    pub async fn say(&mut self, session: &str, text: &str) -> (Vec<Value>, Value) {
        let params = json!({"sessionId": session, "prompt": [{"type": "text", "text": text}]});
        self.call("session/prompt", params).await
    }
}

/// 别的头（另开一条连接）在核心上办一件事：握手、发 `method`，交回回应。
pub async fn elsewhere(home: &Home, method: &str, params: Value) -> Value {
    let (connection, token) = miyu_ipc::connect(&home.root).await.expect("连得上");
    let (read, mut write) = tokio::io::split(connection);
    let mut read = BufReader::new(read).lines();
    let lines = [
        json!({"jsonrpc": "2.0", "id": "else-1", "method": "hello", "params": {
            "protocol": [1, 1], "head": {"kind": "test", "version": "0"}, "token": token}}),
        json!({"jsonrpc": "2.0", "id": "else-2", "method": method, "params": params}),
    ];
    let mut reply = Value::Null;
    for line in lines {
        let line = format!("{line}\n");
        write.write_all(line.as_bytes()).await.expect("写得进");
        let text = within("回应", read.next_line())
            .await
            .expect("读得了")
            .expect("有");
        reply = serde_json::from_str(&text).expect("JSON");
    }
    reply
}

/// 会话 `session`（编号写成字）的日志。
pub fn log(home: &Home, session: &str) -> Vec<miyu_kernel::event::Event> {
    let id = home
        .sessions()
        .into_iter()
        .find(|id| id.to_string() == session)
        .expect("有这个会话");
    home.log(&id)
}

/// 一条 `session/update` 的种类；别的是 `None`。
pub fn kind(message: &Value) -> Option<&str> {
    match message["method"].as_str() {
        Some("session/update") => message["params"]["update"]["sessionUpdate"].as_str(),
        _ => None,
    }
}

/// 几条里某一种 `session/update` 的 `update`，照先后。
pub fn updates<'a>(seen: &'a [Value], wanted: &str) -> Vec<&'a Value> {
    seen.iter()
        .filter(|message| kind(message) == Some(wanted))
        .map(|message| &message["params"]["update"])
        .collect()
}

/// 几条流式的字接起来。
pub fn joined(seen: &[Value], wanted: &str) -> String {
    updates(seen, wanted)
        .iter()
        .filter_map(|update| update["content"]["text"].as_str())
        .collect()
}
