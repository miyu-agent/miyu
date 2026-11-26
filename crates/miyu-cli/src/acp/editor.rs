//! 对着编辑器的那一头（`internal/blueprint/cli/acp.md`「对外的样子」）：标准输出上一行一条 JSON-RPC 2.0，只有协议的行。
//! 回应、出错、`session/update` 通知，和这边向编辑器发的请求（问权限、弹表单），请求的编号从 1 数起。
//!
//! 写不出去（编辑器关了管道）的记一下，之后的都不写：读的那一头随后读到头，整个程序收尾。

use serde_json::{Value, json};
use tokio::io::{AsyncWrite, AsyncWriteExt};

/// 请求的写法不对，或者握手以前来了别的请求。
pub(crate) const INVALID_REQUEST: i64 = -32600;
/// 没有这个方法（规范的 `-32601`），握手时没声明的能力的方法也是。
pub(crate) const METHOD_NOT_FOUND: i64 = -32601;
/// 参数不对：写法不对、模式不认得、配置项不认得。
pub(crate) const INVALID_PARAMS: i64 = -32602;
/// 这边出的错：核心拒了、断了。
pub(crate) const INTERNAL: i64 = -32603;
/// 没有这个会话（规范的 `-32002`，`data.sessionId` 是要的那个）。
pub(crate) const NOT_FOUND: i64 = -32002;

/// 对着编辑器写的那一头。
pub(crate) struct Editor {
    /// 标准输出（测试里是一根管子）。
    out: Box<dyn AsyncWrite + Unpin + Send>,
    /// 下一条请求的编号。
    next: u64,
    /// 写不出去过：之后的都不写。
    broken: bool,
}

impl Editor {
    /// 往 `out` 上写。
    pub(crate) fn new(out: Box<dyn AsyncWrite + Unpin + Send>) -> Editor {
        Editor {
            out,
            next: 0,
            broken: false,
        }
    }

    /// 写一条，一行。
    async fn write(&mut self, message: Value) {
        if self.broken {
            return;
        }
        let line = format!("{message}\n");
        let written = match self.out.write_all(line.as_bytes()).await {
            Ok(()) => self.out.flush().await,
            Err(error) => Err(error),
        };
        if let Err(error) = written {
            tracing::debug!(target: "miyu::acp", %error, "editor gone");
            self.broken = true;
        }
    }

    /// 回一条请求：做成了。
    pub(crate) async fn result(&mut self, id: &Value, result: Value) {
        self.write(json!({"jsonrpc": "2.0", "id": id, "result": result}))
            .await;
    }

    /// 回一条请求：没做成，`message` 照界面语言写，`data` 没有的不写。
    pub(crate) async fn error(
        &mut self,
        id: &Value,
        code: i64,
        message: &str,
        data: Option<Value>,
    ) {
        let mut error = json!({"code": code, "message": message});
        if let Some(data) = data {
            error["data"] = data;
        }
        self.write(json!({"jsonrpc": "2.0", "id": id, "error": error}))
            .await;
    }

    /// 没有这个会话（规范的「Unknown sessions」）：`data.sessionId` 是要的那个。
    pub(crate) async fn unknown(&mut self, id: &Value, session: &str, message: &str) {
        let data = json!({"sessionId": session});
        self.error(id, NOT_FOUND, message, Some(data)).await;
    }

    /// 一条 `session/update`：会话 `session` 的一样变化。
    pub(crate) async fn update(&mut self, session: &str, update: Value) {
        let params = json!({"sessionId": session, "update": update});
        self.write(json!({"jsonrpc": "2.0", "method": "session/update", "params": params}))
            .await;
    }

    /// 向编辑器发一条请求，交回它的编号：回答照这个编号认（[`crate::acp::asking`]）。
    pub(crate) async fn request(&mut self, method: &str, params: Value) -> u64 {
        self.next += 1;
        let id = self.next;
        self.write(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .await;
        id
    }
}

/// 流式的一段字：正文 `agent_message_chunk`、思考 `agent_thought_chunk`、人说的 `user_message_chunk`。`id` 是它属于哪一条
/// （`messageId`，条目编号）：编号变了编辑器另起一块，不接在上一块后面（上一轮打断了的思考、下一轮的思考不连成一句）。
pub(crate) fn chunk(kind: &str, text: &str, id: &str) -> Value {
    json!({"sessionUpdate": kind, "messageId": id, "content": {"type": "text", "text": text}})
}
