//! 客户端（施工 X-1，`docs/blueprint/mcp.md`「怎么走」第一条、第四条）：认时代、握手，之后列工具、调工具。
//!
//! 认时代照 MCP 2026-07-28 版的 stdio 办法：先发 `server/discover`，答得上（`DiscoverResult`，或者说版本不对的
//! `-32022`）的是新时代，每个请求在 `_meta` 里带版本、自己是谁、本事；答了别的错、限定时间里没答的是旧时代，退回
//! `initialize` 握手。MCP 规范留给规范自己用的错误码（`-32099` 到 `-32020`）算新时代的错：照它报，不退回。
//!
//! 新时代的服务在本事里声明了工具、提示词 `listChanged` 的，连上以后在后台开一条 `subscriptions/listen`（施工 PT-1 下）：
//! 只订它声明了的那几种，推来的「变了」照旧数进 [`Client::changes`]；订阅结束了、断了的退避着重开，服务不认这个方法的不再开，
//! 丢下客户端就停。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::watch;

use crate::content::{Called, Content, Listed, Tool};
use crate::error::Failed;
use crate::link::Link;
use crate::progress::Progress;
use crate::prompt::{Message, Prompt, Prompts};
use crate::resource::{self, Page, RESOURCE_FIELDS, TEMPLATE_FIELDS};

mod era;
mod listen;

use listen::listen;

/// 翻页最多翻几页：服务一直给下一页的，到这里停。
const MOST_PAGES: usize = 1000;

/// 只带 `requestState` 的 `input_required` 最多照着重发几回（施工 PT-1 中）：服务一直叫重发的，到这里停。
const MOST_RETRIES: usize = 8;

/// 规范许回 `input_required` 的几个方法（2026-07-28「Multi Round-Trip Requests」）：别的方法回了当写法不对。
const MAY_ASK_INPUT: [&str; 3] = ["tools/call", "prompts/get", "resources/read"];

/// 说哪个时代、哪一版。缓存照它写：下次起来照它直接说。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "era", content = "version", rename_all = "snake_case")]
pub enum Era {
    /// 2026-07-28 起：每个请求自己带版本。
    Modern(String),
    /// 2025-11-25 及更早：先握手。
    Legacy(String),
}

/// 我们是谁：`clientInfo`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hello {
    /// 名字。
    pub name: String,
    /// 版本。
    pub version: String,
}

/// 各等多久。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Waits {
    /// 探 `server/discover` 等多久：旧时代的服务可能不答。
    pub probe: Duration,
    /// 握手、列工具每一次等多久。
    pub answer: Duration,
}

impl Default for Waits {
    /// 探 5 秒，别的 30 秒。
    fn default() -> Waits {
        Waits {
            probe: Duration::from_secs(5),
            answer: Duration::from_secs(30),
        }
    }
}

/// 一个说得上话的 MCP 服务。
pub struct Client {
    link: Arc<Link>,
    era: Era,
    hello: Hello,
    waits: Waits,
    instructions: Option<String>,
    /// 服务声明了提示词（施工 X-39）。
    prompts: bool,
    /// 服务声明了资源（施工 X-41）。
    resources: bool,
    /// 新时代的服务声明了哪几种「变了」（施工 PT-1 下）：`subscriptions/listen` 的 `notifications`，一种都没有的不开。
    listens: Option<Value>,
    /// 后台开着的订阅，丢下客户端就停。
    listening: Option<tokio::task::JoinHandle<()>>,
    /// 下一个进度令牌的编号。
    tokens: AtomicU64,
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Some(listening) = self.listening.take() {
            listening.abort();
        }
    }
}

impl Client {
    /// 经服务的标准输出 `reader`、标准输入 `writer` 说话：认时代、握手。`known` 是上一次认出来的（缓存的）：旧时代的直接握手，
    /// 握不成再重认；新时代的、没有的先探。
    ///
    /// # Errors
    ///
    /// 断了（探的时候服务退出了的也是：旧时代的服务有的收到不认识的请求就退出，调的一方下次照旧时代起它）、等不到、两边
    /// 没有共同的版本、握手被拒。
    pub async fn connect<R, W>(
        reader: R,
        writer: W,
        hello: Hello,
        known: Option<Era>,
        waits: Waits,
    ) -> Result<Client, Failed>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let mut client = Client {
            link: Arc::new(Link::start(reader, writer)),
            era: Era::Legacy(String::new()),
            hello,
            waits,
            instructions: None,
            prompts: false,
            resources: false,
            listens: None,
            listening: None,
            tokens: AtomicU64::new(1),
        };
        let legacy_first = matches!(known, Some(Era::Legacy(_)));
        if !legacy_first || client.shake().await.is_err() {
            client.discover().await?;
        }
        if let (Era::Modern(version), Some(wanted)) = (&client.era, &client.listens) {
            let params = json!({"notifications": wanted, "_meta": client.meta(version)});
            client.listening = Some(tokio::spawn(listen(Arc::clone(&client.link), params)));
        }
        Ok(client)
    }

    /// 认出来的时代和版本。
    pub fn era(&self) -> &Era {
        &self.era
    }

    /// 服务给模型的说明（`instructions`），没给的没有。
    pub fn instructions(&self) -> Option<&str> {
        self.instructions.as_deref()
    }

    /// 服务在本事里声明了提示词（`prompts`，施工 X-39）：声明了的才列。
    pub fn offers_prompts(&self) -> bool {
        self.prompts
    }

    /// 服务在本事里声明了资源（`resources`，施工 X-41）：声明了的才有列、读资源那两件工具。
    pub fn offers_resources(&self) -> bool {
        self.resources
    }

    /// 资源的一页（`resources/list`，施工 X-41）：`cursor` 是上一页交的，第一页不写；每项只留 [`crate::RESOURCE_FIELDS`]，
    /// 没有地址的不收。不限时：同 [`Client::call`]，调的一方自己限。
    ///
    /// # Errors
    ///
    /// 断了、服务报错、回的写法不对。
    pub async fn resources(&self, cursor: Option<&str>) -> Result<Page, Failed> {
        self.page("resources/list", "resources", cursor, &RESOURCE_FIELDS)
            .await
    }

    /// 地址模板的一页（`resources/templates/list`，施工 X-41）：同 [`Client::resources`]，每项只留 [`crate::TEMPLATE_FIELDS`]。
    ///
    /// # Errors
    ///
    /// 断了、服务报错、回的写法不对。
    pub async fn resource_templates(&self, cursor: Option<&str>) -> Result<Page, Failed> {
        self.page(
            "resources/templates/list",
            "resourceTemplates",
            cursor,
            &TEMPLATE_FIELDS,
        )
        .await
    }

    /// 读资源 `uri`（`resources/read`，施工 X-41）：交回几段内容，都是 [`Content::Resource`]。不限时，调的一方自己限。
    ///
    /// # Errors
    ///
    /// 断了、服务报错（没有这个资源）、回的写法不对。
    pub async fn read_resource(&self, uri: &str) -> Result<Vec<Content>, Failed> {
        let mut params = Map::new();
        params.insert("uri".to_string(), Value::String(uri.to_string()));
        let result = self.ask("resources/read", params, None).await?;
        resource::contents(&result)
            .ok_or_else(|| Failed::Malformed("resources/read without contents".to_string()))
    }

    /// `method` 的一页：`key` 那一列，每项只留 `fields`。
    async fn page(
        &self,
        method: &str,
        key: &str,
        cursor: Option<&str>,
        fields: &[&str],
    ) -> Result<Page, Failed> {
        let mut params = Map::new();
        if let Some(cursor) = cursor {
            params.insert("cursor".to_string(), Value::String(cursor.to_string()));
        }
        let result = self.ask(method, params, None).await?;
        resource::page(&result, key, fields)
            .ok_or_else(|| Failed::Malformed(format!("{method} without {key}")))
    }

    /// 列全工具（`tools/list`，照 `nextCursor` 翻页）。写法不对的那几项不收，记在 [`Listed::skipped`]。
    ///
    /// # Errors
    ///
    /// 断了、等不到、服务报错、回的写法不对。
    pub async fn tools(&self) -> Result<Listed, Failed> {
        let mut listed = Listed {
            tools: Vec::new(),
            skipped: Vec::new(),
            fresh_for: None,
        };
        let mut cursor: Option<String> = None;
        for page in 0..MOST_PAGES {
            let mut params = Map::new();
            if let Some(cursor) = cursor.take() {
                params.insert("cursor".to_string(), Value::String(cursor));
            }
            let result = self
                .ask("tools/list", params, Some(self.waits.answer))
                .await?;
            let Some(tools) = result.get("tools").and_then(Value::as_array) else {
                return Err(Failed::Malformed("tools/list without tools".to_string()));
            };
            for raw in tools {
                match Tool::read(raw) {
                    Ok(tool) => listed.tools.push(tool),
                    Err(problem) => listed.skipped.push(problem),
                }
            }
            if page == 0 {
                listed.fresh_for = result
                    .get("ttlMs")
                    .and_then(Value::as_u64)
                    .map(Duration::from_millis);
            }
            match result.get("nextCursor").and_then(Value::as_str) {
                Some(next) if !next.is_empty() => cursor = Some(next.to_string()),
                _ => return Ok(listed),
            }
        }
        Err(Failed::Malformed(format!(
            "tools/list went past {MOST_PAGES} pages"
        )))
    }

    /// 列全提示词（`prompts/list`，照 `nextCursor` 翻页，施工 X-39）。写法不对的那几项不收，记在 [`Prompts::skipped`]。
    ///
    /// # Errors
    ///
    /// 断了、等不到、服务报错、回的写法不对。
    pub async fn prompts(&self) -> Result<Prompts, Failed> {
        let mut listed = Prompts {
            prompts: Vec::new(),
            skipped: Vec::new(),
            fresh_for: None,
        };
        let mut cursor: Option<String> = None;
        for page in 0..MOST_PAGES {
            let mut params = Map::new();
            if let Some(cursor) = cursor.take() {
                params.insert("cursor".to_string(), Value::String(cursor));
            }
            let result = self
                .ask("prompts/list", params, Some(self.waits.answer))
                .await?;
            let Some(prompts) = result.get("prompts").and_then(Value::as_array) else {
                return Err(Failed::Malformed(
                    "prompts/list without prompts".to_string(),
                ));
            };
            for raw in prompts {
                match Prompt::read(raw) {
                    Ok(prompt) => listed.prompts.push(prompt),
                    Err(problem) => listed.skipped.push(problem),
                }
            }
            if page == 0 {
                listed.fresh_for = result
                    .get("ttlMs")
                    .and_then(Value::as_u64)
                    .map(Duration::from_millis);
            }
            match result.get("nextCursor").and_then(Value::as_str) {
                Some(next) if !next.is_empty() => cursor = Some(next.to_string()),
                _ => return Ok(listed),
            }
        }
        Err(Failed::Malformed(format!(
            "prompts/list went past {MOST_PAGES} pages"
        )))
    }

    /// 取提示词 `name`，参数 `arguments`（名字到字，施工 X-39）：交回几条消息，照先后。不限时：同 [`Client::call`]，调的一方
    /// 自己限。
    ///
    /// # Errors
    ///
    /// 断了、服务报错（名字不对、缺参数）、回的写法不对、要补信息再问。
    pub async fn prompt(
        &self,
        name: &str,
        arguments: &BTreeMap<String, String>,
    ) -> Result<Vec<Message>, Failed> {
        let mut params = Map::new();
        params.insert("name".to_string(), Value::String(name.to_string()));
        params.insert("arguments".to_string(), json!(arguments));
        let result = self.ask("prompts/get", params, None).await?;
        Ok(Message::read_all(&result))
    }

    /// 调工具 `name`，参数 `arguments`（一个对象）。不限时：调的一方自己限，限到了丢掉这个 future，服务收到取消。
    ///
    /// # Errors
    ///
    /// 断了、服务报错（工具不存在、参数不对）、回的写法不对、要补信息再问。工具自己说出错了不算：照 [`Called::is_error`]。
    pub async fn call(&self, name: &str, arguments: Value) -> Result<Called, Failed> {
        let mut params = Map::new();
        params.insert("name".to_string(), Value::String(name.to_string()));
        params.insert("arguments".to_string(), arguments);
        let result = self.ask("tools/call", params, None).await?;
        Ok(Called::read(&result))
    }

    /// 同 [`Client::call`]，另带一个进度令牌，服务推来的进度一条条交给 `report`（施工 PT-1 下）。服务不推的就不交。
    ///
    /// # Errors
    ///
    /// 同 [`Client::call`]。
    pub async fn call_reporting(
        &self,
        name: &str,
        arguments: Value,
        report: impl Fn(Progress),
    ) -> Result<Called, Failed> {
        let token = json!(format!("p{}", self.tokens.fetch_add(1, Ordering::Relaxed)));
        let (mut updates, _following) = self.link.follow(&token);
        let mut params = Map::new();
        params.insert("name".to_string(), Value::String(name.to_string()));
        params.insert("arguments".to_string(), arguments);
        params.insert("_meta".to_string(), json!({"progressToken": token}));
        let asked = self.ask("tools/call", params, None);
        tokio::pin!(asked);
        let tell = |update: &Value| {
            if let Some(progress) = Progress::read(update) {
                report(progress);
            }
        };
        loop {
            tokio::select! {
                result = &mut asked => {
                    // 读的一头照先后交：回应到了的时候，这次的进度都已经在通道里了，先交完再回。
                    while let Ok(update) = updates.try_recv() {
                        tell(&update);
                    }
                    return result.map(|result| Called::read(&result));
                }
                Some(update) = updates.recv() => tell(&update),
            }
        }
    }

    /// 服务说工具变了几次：变了就再列一次。
    pub fn changes(&self) -> watch::Receiver<u64> {
        self.link.changes()
    }

    /// 等到连接断了（服务退出了、关了输出）。
    pub async fn closed(&self) {
        self.link.closed().await;
    }

    /// 照认出来的时代发一个请求：新时代的参数里带 `_meta`、结果要是完整的。
    ///
    /// 新时代的服务回 `input_required`（多轮请求，施工 PT-1 中）：只带 `requestState`、不要客户端补东西的（规范的例子是服务忙时
    /// 分流），换个编号把原请求连同原样的 `requestState` 再发一遍，至多 [`MOST_RETRIES`] 回；要补东西的（`inputRequests` 不空）
    /// 照旧报 [`Failed::InputRequired`]：我们没声明能补的本事，守规矩的服务不该这样回。只有 [`MAY_ASK_INPUT`] 那几个方法许这样回。
    async fn ask(
        &self,
        method: &str,
        mut params: Map<String, Value>,
        wait: Option<Duration>,
    ) -> Result<Value, Failed> {
        if let Era::Modern(version) = &self.era {
            // 调的一方已经写了的（进度令牌）留着，并上版本、自己是谁、本事。
            let meta = params.entry("_meta").or_insert_with(|| json!({}));
            if let (Some(meta), Value::Object(ours)) = (meta.as_object_mut(), self.meta(version)) {
                meta.extend(ours);
            }
        }
        for _ in 0..=MOST_RETRIES {
            let result = self
                .link
                .ask(method, Value::Object(params.clone()), wait, true)
                .await?;
            match result.get("resultType").and_then(Value::as_str) {
                None | Some("complete") => return Ok(result),
                Some("input_required") if MAY_ASK_INPUT.contains(&method) => {
                    let wants_input = result
                        .get("inputRequests")
                        .and_then(Value::as_object)
                        .is_some_and(|asked| !asked.is_empty());
                    match result.get("requestState").and_then(Value::as_str) {
                        Some(state) if !wants_input => {
                            params.insert("requestState".to_string(), json!(state));
                        }
                        _ => return Err(Failed::InputRequired),
                    }
                }
                Some(other) => {
                    return Err(Failed::Malformed(format!(
                        "{method} answered with resultType {other}"
                    )));
                }
            }
        }
        Err(Failed::Malformed(format!(
            "{method} kept asking to be sent again"
        )))
    }

    /// 新时代每个请求带的 `_meta`。
    fn meta(&self, version: &str) -> Value {
        json!({
            "io.modelcontextprotocol/protocolVersion": version,
            "io.modelcontextprotocol/clientInfo": {"name": self.hello.name, "version": self.hello.version},
            "io.modelcontextprotocol/clientCapabilities": {},
        })
    }
}
