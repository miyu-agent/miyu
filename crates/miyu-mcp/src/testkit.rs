//! 测试用的假 MCP 服务（施工 X-1）：在进程里经一对内存管道照剧本演，新旧两个时代、几种不好好说话的都演得出。记下收到的
//! 每一条，测试照它查客户端发了什么。
//!
//! 几件工具：`echo` 照参数 `text` 回文字；`picture` 回一张图和一句话；`fail` 说自己出错了；`structured` 只回结构化的；
//! `slow` 不回（等取消）；`ask` 先反过来问客户端要东西，等它回了再答；`pinged` 先反过来 `ping` 客户端，照它回没回成答；
//! `input` 要补信息（新时代）；`die` 一收到就退出。别的名字回「不认识这件工具」（`-32602`）。
//!
//! 剧本带了资源的（施工 X-41）照 `resources.rs`。
//!
//! 剧本带了提示词的（施工 X-39），本事里声明 `prompts`：`review` 要参数 `code`（必填）、`focus`（可选），回一句、一张图、她说的
//! 一句；`hello` 不要参数，回一句；缺必填的、别的名字回 `-32602`。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Value, json};
use tokio::io::{AsyncWriteExt, BufReader, DuplexStream, ReadHalf, WriteHalf};

use crate::wire::{self, Read};

mod calls;
pub mod http;
pub mod oauth;
mod resources;

pub use resources::{resources, templates};

/// 演哪个时代、怎么对待 `server/discover`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 新时代：答 `server/discover`，每个请求要带认得的版本，不认 `initialize`。
    Modern,
    /// 新时代，但答 `server/discover` 说缺客户端的本事（`-32021`，规范留给自己的错）。
    ModernDemands,
    /// 旧时代：`server/discover` 回「没有这个方法」。
    LegacyRefuses,
    /// 旧时代：`server/discover` 不回。
    LegacySilent,
    /// 旧时代：收到 `server/discover` 就退出。
    LegacyDies,
}

/// 新时代的订阅（施工 PT-1 下）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listen {
    /// 不声明 `listChanged`：客户端不开订阅。
    Off,
    /// 声明工具的 `listChanged`；`subscriptions/listen` 来了先回确认，再马上推一条工具变了（带订阅编号），之后不回。
    On,
    /// 声明了，但 `subscriptions/listen` 回「没有这个方法」。
    Refused,
}

/// 剧本。
#[derive(Debug, Clone)]
pub struct Script {
    /// 哪个时代。
    pub kind: Kind,
    /// 新时代：认得的版本；旧时代：握手时答第一个。
    pub versions: Vec<String>,
    /// `tools/list` 列的，原样。
    pub tools: Vec<Value>,
    /// 一页几件，0 是一页列完。
    pub page: usize,
    /// 新时代的 `ttlMs`。
    pub ttl_ms: Option<u64>,
    /// 第一次列完以后说一次工具变了。
    pub change_after_list: bool,
    /// 给模型的说明。
    pub instructions: Option<String>,
    /// `prompts/list` 列的，原样（施工 X-39）；空的不声明 `prompts`。
    pub prompts: Vec<Value>,
    /// 第一次列完提示词以后说一次提示词变了（施工 X-39）。
    pub change_prompts_after_list: bool,
    /// `resources/list` 列的，原样（施工 X-41）；和模板都是空的不声明 `resources`。
    pub resources: Vec<Value>,
    /// `resources/templates/list` 列的，原样（施工 X-41）。
    pub resource_templates: Vec<Value>,
    /// 新时代的订阅（施工 PT-1 下）。
    pub listen: Listen,
}

impl Script {
    /// 新时代，认 `2026-07-28`，带上面那几件工具。
    pub fn modern() -> Script {
        Script {
            kind: Kind::Modern,
            versions: vec!["2026-07-28".to_string()],
            tools: tools(),
            page: 0,
            ttl_ms: None,
            change_after_list: false,
            instructions: None,
            prompts: Vec::new(),
            change_prompts_after_list: false,
            resources: Vec::new(),
            resource_templates: Vec::new(),
            listen: Listen::Off,
        }
    }

    /// 旧时代 `kind`，握手答 `2025-11-25`，带上面那几件工具。
    pub fn legacy(kind: Kind) -> Script {
        Script {
            kind,
            versions: vec!["2025-11-25".to_string()],
            ..Script::modern()
        }
    }
}

/// 剧本里的几件工具。
pub fn tools() -> Vec<Value> {
    [
        "echo",
        "picture",
        "fail",
        "structured",
        "slow",
        "ask",
        "input",
        "die",
    ]
    .iter()
    .map(|name| {
        json!({
            "name": name,
            "description": format!("The {name} tool."),
            "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}},
            "annotations": {"readOnlyHint": *name == "echo"},
        })
    })
    .collect()
}

/// 剧本里的几个提示词（施工 X-39）：`review`、`hello`，外加一项没有名字的（写法不对，客户端不收）。
pub fn prompts() -> Vec<Value> {
    vec![
        json!({
            "name": "review",
            "title": "Review code",
            "description": "Review a piece of code.",
            "arguments": [
                {"name": "code", "description": "The code", "required": true},
                {"name": "focus", "description": "What to look at"},
            ],
        }),
        json!({"description": "No name."}),
        json!({"name": "hello"}),
    ]
}

/// 演着的假服务：查它收到了什么、演完了没有。
#[derive(Clone, Default)]
pub struct Fake {
    received: Arc<Mutex<Vec<Value>>>,
    ended: Arc<AtomicBool>,
}

impl Fake {
    /// 演完了：客户端关了它的输入，或者剧本让它退出了。
    pub fn ended(&self) -> bool {
        self.ended.load(Ordering::SeqCst)
    }

    /// 收到的每一条，照先后。
    pub fn received(&self) -> Vec<Value> {
        self.received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 收到的每一条的方法，回应（没有方法的）写 `answer`。
    pub fn methods(&self) -> Vec<String> {
        self.received()
            .iter()
            .map(|message| {
                message
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or("answer")
                    .to_string()
            })
            .collect()
    }

    fn note(&self, message: Value) {
        self.received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(message);
    }
}

/// 起一个假服务：交回客户端读的一头、写的一头，和查它的 [`Fake`]。
pub fn start(script: Script) -> (ReadHalf<DuplexStream>, WriteHalf<DuplexStream>, Fake) {
    let (ours, theirs) = tokio::io::duplex(1 << 20);
    let (client_read, client_write) = tokio::io::split(ours);
    let fake = Fake::default();
    tokio::spawn(serve(theirs, script, fake.clone()));
    (client_read, client_write, fake)
}

/// 一直演到客户端断开、或者剧本让它退出；演完了记下。
async fn serve(stream: DuplexStream, script: Script, fake: Fake) {
    play(stream, script, &fake).await;
    fake.ended.store(true, Ordering::SeqCst);
}

/// 照剧本演：收一条答一条。
async fn play(stream: DuplexStream, script: Script, fake: &Fake) {
    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);
    let mut stage = Stage::default();
    while let Ok(Read::Line(line)) = wire::read_line(&mut reader).await {
        let Ok(message) = serde_json::from_slice::<Value>(&line) else {
            continue;
        };
        fake.note(message.clone());
        let Some(out) = stage.answer(&script, &message) else {
            return;
        };
        for line in out {
            if writer.write_all(&line).await.is_err() || writer.flush().await.is_err() {
                return;
            }
        }
    }
}

/// 演到哪了。
#[derive(Default)]
struct Stage {
    /// 说过工具变了。
    changed: bool,
    /// 说过提示词变了（施工 X-39）。
    prompts_changed: bool,
    /// `ask` 那一次在等客户端回的调用编号。
    asking: Option<Value>,
}

impl Stage {
    /// 收到一条，交回要写出去的几行；交回空的是退出。
    fn answer(&mut self, script: &Script, message: &Value) -> Option<Vec<Vec<u8>>> {
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            // 客户端回了我们反过来问的：`ask`、`pinged` 那一次接着答，`pinged` 照它回没回成说。
            return Some(match (message.get("id"), self.asking.take()) {
                (Some(id), Some(call)) if id == "s1" => {
                    vec![complete(script, &call, text("asked"))]
                }
                (Some(id), Some(call)) if id == "p1" => {
                    let said = match message.get("result") {
                        Some(_) => "pong",
                        None => "no pong",
                    };
                    vec![complete(script, &call, text(said))]
                }
                _ => Vec::new(),
            });
        };
        let Some(id) = message.get("id").cloned() else {
            return Some(Vec::new());
        };
        let modern = matches!(script.kind, Kind::Modern | Kind::ModernDemands);
        if method == "server/discover" {
            return self.discover(script, &id, message);
        }
        if method == "initialize" {
            return Some(vec![match modern {
                true => wire::answer(&id, Err((-32601, "Method not found"))),
                false => wire::answer(
                    &id,
                    Ok(json!({
                        "protocolVersion": script.versions[0],
                        "capabilities": capabilities(script),
                        "serverInfo": {"name": "fake", "version": "0"},
                        "instructions": script.instructions,
                    })),
                ),
            }]);
        }
        if modern && let Some(refused) = wrong_version(script, &id, message) {
            return Some(vec![refused]);
        }
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        match method {
            "tools/list" => Some(self.list(script, &id, &params)),
            "tools/call" => self.call(script, &id, &params),
            "subscriptions/listen" if modern && script.listen == Listen::On => {
                let tagged = json!({"io.modelcontextprotocol/subscriptionId": id});
                Some(vec![
                    wire::notice(
                        "notifications/subscriptions/acknowledged",
                        json!({"notifications": params["notifications"], "_meta": tagged}),
                    ),
                    wire::notice("notifications/tools/list_changed", json!({"_meta": tagged})),
                ])
            }
            "prompts/list" if !script.prompts.is_empty() => {
                Some(self.prompts(script, &id, &params))
            }
            "prompts/get" if !script.prompts.is_empty() => Some(vec![prompt(script, &id, &params)]),
            "resources/list" if resources::offered(script) => {
                Some(vec![resources::page(script, &id, &params, "resources")])
            }
            "resources/templates/list" if resources::offered(script) => {
                Some(vec![resources::page(
                    script,
                    &id,
                    &params,
                    "resourceTemplates",
                )])
            }
            "resources/read" if resources::offered(script) => {
                Some(vec![resources::read(script, &id, &params)])
            }
            _ => Some(vec![wire::answer(&id, Err((-32601, "Method not found")))]),
        }
    }

    fn discover(&self, script: &Script, id: &Value, message: &Value) -> Option<Vec<Vec<u8>>> {
        match script.kind {
            Kind::Modern => Some(vec![wrong_version(script, id, message).unwrap_or_else(|| {
                wire::answer(
                    id,
                    Ok(json!({
                        "resultType": "complete",
                        "supportedVersions": script.versions,
                        "capabilities": capabilities(script),
                        "_meta": {"io.modelcontextprotocol/serverInfo": {"name": "fake", "version": "0"}},
                        "instructions": script.instructions,
                    })),
                )
            })]),
            Kind::ModernDemands => Some(vec![wire::answer(id, Err((-32021, "Missing required client capability")))]),
            Kind::LegacyRefuses => Some(vec![wire::answer(id, Err((-32601, "Method not found")))]),
            Kind::LegacySilent => Some(Vec::new()),
            Kind::LegacyDies => None,
        }
    }

    fn list(&mut self, script: &Script, id: &Value, params: &Value) -> Vec<Vec<u8>> {
        let start = params
            .get("cursor")
            .and_then(Value::as_str)
            .and_then(|cursor| cursor.parse::<usize>().ok())
            .unwrap_or(0);
        let size = match script.page {
            0 => script.tools.len(),
            page => page,
        };
        let end = (start + size).min(script.tools.len());
        let mut result = json!({"tools": script.tools.get(start..end).unwrap_or_default()});
        if end < script.tools.len() {
            result["nextCursor"] = json!(end.to_string());
        }
        if let Some(ttl) = script.ttl_ms {
            result["ttlMs"] = json!(ttl);
        }
        let mut out = vec![complete(script, id, result)];
        if script.change_after_list && !self.changed && end >= script.tools.len() {
            self.changed = true;
            out.push(wire::notice("notifications/tools/list_changed", json!({})));
        }
        out
    }

    /// `prompts/list`：同工具翻页；第一次列完照剧本说一次提示词变了。
    fn prompts(&mut self, script: &Script, id: &Value, params: &Value) -> Vec<Vec<u8>> {
        let start = params
            .get("cursor")
            .and_then(Value::as_str)
            .and_then(|cursor| cursor.parse::<usize>().ok())
            .unwrap_or(0);
        let size = match script.page {
            0 => script.prompts.len(),
            page => page,
        };
        let end = (start + size).min(script.prompts.len());
        let mut result = json!({"prompts": script.prompts.get(start..end).unwrap_or_default()});
        if end < script.prompts.len() {
            result["nextCursor"] = json!(end.to_string());
        }
        if let Some(ttl) = script.ttl_ms {
            result["ttlMs"] = json!(ttl);
        }
        let mut out = vec![complete(script, id, result)];
        if script.change_prompts_after_list && !self.prompts_changed && end >= script.prompts.len()
        {
            self.prompts_changed = true;
            out.push(wire::notice(
                "notifications/prompts/list_changed",
                json!({}),
            ));
        }
        out
    }
}

/// 声明的本事：工具，带了提示词的另有 `prompts`（施工 X-39），带了资源的另有 `resources`（施工 X-41）。
fn capabilities(script: &Script) -> Value {
    // 施工 PT-1 下：只有开了订阅的剧本声明 `listChanged`，别的剧本新时代的客户端不开订阅、收到的照旧。
    let listens = script.listen != Listen::Off;
    let mut capabilities = json!({"tools": {"listChanged": listens}});
    if !script.prompts.is_empty() {
        capabilities["prompts"] = json!({"listChanged": listens});
    }
    if resources::offered(script) {
        capabilities["resources"] = json!({});
    }
    capabilities
}

/// `prompts/get`：照名字和参数回几条消息；缺必填的、别的名字回 `-32602`。
fn prompt(script: &Script, id: &Value, params: &Value) -> Vec<u8> {
    let arguments = &params["arguments"];
    let said =
        |role: &str, text: String| json!({"role": role, "content": {"type": "text", "text": text}});
    let messages = match params["name"].as_str().unwrap_or_default() {
        "review" => {
            let Some(code) = arguments["code"].as_str() else {
                return wire::answer(id, Err((-32602, "Missing required argument: code")));
            };
            let focus = arguments["focus"]
                .as_str()
                .map(|focus| format!(" for {focus}"))
                .unwrap_or_default();
            json!([
                said("user", format!("Review this code{focus}:\n{code}")),
                {"role": "user", "content": {"type": "image", "data": "aGk=", "mimeType": "image/png"}},
                said("assistant", "Sure.".to_string()),
            ])
        }
        "hello" => json!([said("user", "Say hello.".to_string())]),
        _ => return wire::answer(id, Err((-32602, "Unknown prompt"))),
    };
    complete(
        script,
        id,
        json!({"description": "A prompt.", "messages": messages}),
    )
}

/// 一段文字的结果。
fn text(text: &str) -> Value {
    json!({"content": [{"type": "text", "text": text}]})
}

/// 回一个成了的结果：新时代的补上 `resultType`（已经写了的不动）。
fn complete(script: &Script, id: &Value, mut result: Value) -> Vec<u8> {
    let modern = matches!(script.kind, Kind::Modern | Kind::ModernDemands);
    if modern && result.get("resultType").is_none() {
        result["resultType"] = json!("complete");
    }
    wire::answer(id, Ok(result))
}

/// 新时代：请求带的版本不在认得的里面，回 `-32022`。
fn wrong_version(script: &Script, id: &Value, message: &Value) -> Option<Vec<u8>> {
    let requested = message["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"]
        .as_str()
        .unwrap_or_default();
    if script.versions.iter().any(|version| version == requested) {
        return None;
    }
    Some(wire::answer_with(
        id,
        -32022,
        "Unsupported protocol version",
        json!({"supported": script.versions, "requested": requested}),
    ))
}
