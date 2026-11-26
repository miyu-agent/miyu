//! 到核心的连接断了以后再连上（施工 ACP-9，`internal/blueprint/cli/acp.md`「核心断了」）：核心停了、换了、崩了，`miyu acp` 不退出。
//!
//! 1. 断的那一下：等着回的 `session/prompt` 都照出错回（规范里 `cancelled` 只给编辑器自己打断的），问编辑器还没等到回答的
//!    确认、提问作废（核心那边的已经没了）。
//! 2. 当场照头自己重连的连法连一次（[`Reconnect::connect`] 的 `asked` 是假，`miyu_ipc::reconnect_or_start`）：崩了的拉起来，
//!    人停的（`miyu core stop`）不拉。连不上的先这样放着，不轮询。
//! 3. 还没连上、编辑器又要用核心（`initialize`、`session/close` 以外的请求）：照 `miyu ask` 的连法连（`asked` 是真，
//!    `miyu_ipc::connect_or_start`，核心没开就拉起）。连不上的那一条请求回出错，之后的请求再试。
//! 4. 连上了：重新握手，订配置的推送（模型列表变了要知道，`choosing.rs`），照编辑器里还开着的会话一个个重新订阅视图流，订阅
//!    回应里的条目照「正在发生」再过一遍（同掉队以后的 `resync`），再重取一次能选的模型。

use std::future::Future;
use std::pin::Pin;
use std::process::Command;

use serde_json::{Value, json};

use miyu_ipc::Connection;
use miyu_store::root::DataRoot;

use super::editor;
use super::server::Server;
use crate::language::Language;
use crate::rpc::Rpc;

/// 连一次的结果：连接和本机令牌，连不上的是给人看的一句。
pub type Linked = Result<(Connection, String), String>;

/// 怎么再连上核心。`miyu acp` 自己用的是本机的数据根（`relink.rs` 的 `Local`）；测试在进程里起核心，自己给一个。
pub trait Reconnect: Send {
    /// 连一次：`asked` 是编辑器这时要用核心（核心没开就拉起，人停的也拉）；不是的是断了以后自己重连（人停的不拉）。
    fn connect(&mut self, asked: bool) -> Pin<Box<dyn Future<Output = Linked> + Send + '_>>;
}

/// 本机的数据根上连：拉起核心的命令照 `start`。
pub(crate) struct Local<F> {
    /// 数据根。
    pub(crate) root: DataRoot,
    /// 拉起核心的命令。
    pub(crate) start: F,
}

impl<F: Fn() -> Command + Send + Sync> Reconnect for Local<F> {
    fn connect(&mut self, asked: bool) -> Pin<Box<dyn Future<Output = Linked> + Send + '_>> {
        Box::pin(async move {
            let start = || (self.start)();
            let linked = match asked {
                true => miyu_ipc::connect_or_start(&self.root, start).await,
                false => miyu_ipc::reconnect_or_start(&self.root, start).await,
            };
            linked.map_err(|error| crate::startup::spoken(&error))
        })
    }
}

/// 握手：本机令牌 `token`，握手以前的界面语言 `language`。交回核心的回应。
///
/// # Errors
///
/// 被拒绝、核心断开：核心的原话（断开的是空的）。
pub(crate) async fn hello(rpc: &mut Rpc, token: &str, language: Language) -> Result<Value, Value> {
    let hello = json!({
        "protocol": [1, 1],
        "head": {"kind": "acp", "version": env!("CARGO_PKG_VERSION")},
        "locale": language.locale(),
        "caps": {"input": true},
        "token": token,
    });
    super::prompt::answered(rpc.call("hello", hello).await)
}

impl Server {
    /// 握好手以后：照回应记下界面语言、核心所在机器的家目录，订配置的推送。
    pub(crate) async fn greeted(&mut self, hello: &Value) {
        self.language = crate::link::spoken(hello, self.language);
        self.home = hello["host"]["home"].as_str().map(str::to_string);
        let _sent = self
            .rpc
            .send("subscribe", json!({"stream": "config"}))
            .await;
    }

    /// 核心断了：等着回的 `session/prompt` 都照出错回，问编辑器的作废，当场自己重连一次。
    pub(crate) async fn core_gone(&mut self) {
        self.linked = false;
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
        self.asks.clear();
        for session in self.sessions.values_mut() {
            session.asked.clear();
        }
        tracing::info!(target: "miyu::acp", "core disconnected");
        if let Err(error) = self.relink(false).await {
            tracing::info!(target: "miyu::acp", error = error.as_str(), "core not reconnected");
        }
    }

    /// 编辑器要用核心：没连着的照 `miyu ask` 的连法连上。连不上的回 `id` 那一条出错，交回假。主循环还没顾上的断开（编辑器的
    /// 这一行和断开同时到）先办完：剩下的推送照常翻，再照断了办。
    pub(crate) async fn linked_for(&mut self, id: &Value) -> bool {
        if self.linked && self.rpc.closed() {
            while let Some(message) = self.rpc.next().await {
                self.core(message).await;
            }
            self.core_gone().await;
        }
        if self.linked {
            return true;
        }
        match self.relink(true).await {
            Ok(()) => true,
            Err(message) => {
                self.editor
                    .error(id, editor::INTERNAL, &message, None)
                    .await;
                false
            }
        }
    }

    /// 连上、握手、订配置，开着的会话一个个重新订阅，重取能选的模型。
    ///
    /// # Errors
    ///
    /// 连不上、握手被拒：给人看的一句。
    async fn relink(&mut self, asked: bool) -> Result<(), String> {
        let (connection, token) = self.reconnect.connect(asked).await?;
        let mut rpc = Rpc::new(connection, "acp");
        let hello = hello(&mut rpc, &token, self.language)
            .await
            .map_err(|error| {
                tracing::warn!(target: "miyu::acp", %error, "hello refused");
                self.refused(&error)
            })?;
        self.rpc = rpc;
        self.linked = true;
        self.greeted(&hello).await;
        tracing::info!(target: "miyu::acp", sessions = self.sessions.len(), "core reconnected");
        let open: Vec<String> = self.sessions.keys().cloned().collect();
        for session in open {
            self.resync(&session).await;
        }
        self.refresh_choices().await;
        Ok(())
    }
}
