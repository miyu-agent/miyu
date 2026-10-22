//! 连接从哪来（施工 Y-1 下，设计 `06-多用户与身份.md` 第八补第 5 条、U18，2026-10-22 项目主人定）：本机、本机的另一个人、远程。
//! 读核心那台机器的文件、交本机路径、打开本机文件夹这一类方法只给本机的连接（[`local_machine_only`]），别的回
//! `local_machine_only`；握手回应不给本机的 `host`。这一版不开网络，先把「本机 = 可信」的假设拆掉。

use std::sync::Arc;

use serde_json::Value;
use tokio::io::{AsyncRead, AsyncWrite};

use super::serve_from;
use crate::Core;
use crate::refusal::Refusal;
use crate::wire::{Notice, Request};

/// 一个连接从哪来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reach {
    /// 本机：主入口（只有核心的主人连得上的本机套接字、命名管道）、核心拉起的扩展。
    Local,
    /// 本机的另一个人：第二个入口（谁都能连的那一个），不是管理员本人。
    Public,
    /// 远程：网页软件替远程的人开的连接，握手之前说了 `connection.remote`；`from` 是它说的来自哪个地址，照原样记。
    Remote {
        /// 来自哪个地址。
        from: String,
    },
}

impl Reach {
    /// 是不是本机：本机才能用的方法只给它。
    pub fn is_local(&self) -> bool {
        *self == Reach::Local
    }

    /// 握手之前听到的通知 `notice`：网页软件说的 `connection.remote {from}` 把这个连接标成远程，记一行。只能往下标：标了就变不回
    /// 本机，远程的再说一次照旧是第一次说的那个地址。握手以后说的不理（通知不回应）。交回标了没有。
    pub(crate) fn heard(&mut self, notice: &Notice, shaken: bool) -> bool {
        if shaken || notice.method != REMOTE || matches!(self, Reach::Remote { .. }) {
            return false;
        }
        tracing::info!(target: "miyu::endpoint", "connection marked remote");
        let from = notice
            .params
            .get("from")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        *self = Reach::Remote { from };
        true
    }
}

/// 同 [`super::serve`]，这个连接从 `reach` 来（施工 Y-1 下）：第二个本机入口交 [`Reach::Public`]；测试里造远程的连接也走它。
#[doc(hidden)]
pub async fn serve_with<S>(stream: S, core: Arc<Core>, reach: Reach)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    serve_from(stream, core, None, reach).await;
}

/// 网页软件标远程的那一句。
pub(crate) const REMOTE: &str = "connection.remote";

/// 从 `reach` 来的这一条请求调不调得了：本机才能用的，不是本机来的拒。
pub(super) fn refuse(reach: &Reach, request: &Request) -> Option<Refusal> {
    (!reach.is_local() && local_machine_only(&request.method, &request.params))
        .then_some(Refusal::LOCAL_MACHINE_ONLY)
}

/// 握手的回应 `result`：不是本机来的不给 `host`（核心那台机器上的家目录、工作区这些绝对路径）。
pub(super) fn strip(reach: &Reach, result: &mut Value) {
    if !reach.is_local()
        && let Some(result) = result.as_object_mut()
    {
        result.remove("host");
    }
}

/// 方法 `method`（参数 `params`）只给本机的连接（施工 Y-1 下，照 2026-10-22 的代码盘的；主会话同一天定带本机路径参数的整条拒，
/// 只在回应里带本机路径的这一步不动）：
///
/// - 读、找核心那台机器的文件，开终端会话，OAuth 回调听核心的 `127.0.0.1`，首次设置码；
/// - 交本机路径的：`blob.put` 的 `path`、`package.install` 的 `path`、`kb.mount` 的 `folder`、`mcp.set` 的 `command`（在核心那台
///   机器上起程序）、`check` 的 `file`、`cwd`、`config.trust`、`config.get` 的 `cwd`；
/// - 选核心那台机器上的目录：会话的 `cwd`、`dirs`，附件的 `path`，`/workspace <路径>`，项目的 `workspace`。远程来的会话用它
///   自己的默认工作区（`session.create` 不写 `cwd`）。
pub(crate) fn local_machine_only(method: &str, params: &Value) -> bool {
    let has = |key: &str| params.get(key).is_some_and(|value| !value.is_null());
    let listed = |key: &str| {
        params
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
    };
    let attached_path = || {
        params
            .get("attachments")
            .and_then(Value::as_array)
            .is_some_and(|items| items.iter().any(|item| item.get("path").is_some()))
    };
    match method {
        "fs.list"
        | "fs.find"
        | "fs.realpath"
        | "fs.read"
        | "session.terminal"
        | "session.set_terminal"
        | "mcp.login"
        | "account.setup_code" => true,
        "blob.put" | "package.install" => has("path"),
        "kb.mount" => has("folder"),
        "mcp.set" => has("command"),
        "check" => has("file") || has("cwd"),
        "config.trust" | "config.get" => has("cwd"),
        "session.create" | "session.set_workspace" => has("cwd") || listed("dirs"),
        "session.send" => has("cwd") || listed("dirs") || attached_path(),
        "session.redo" => attached_path(),
        "command.run" => has("cwd") || moves(params),
        "project.create" | "project.update" => has("workspace"),
        _ => false,
    }
}

/// `command.run` 的原文是 `/workspace <路径>`：换到核心那台机器上的一个目录。不带路径的只说现在在哪，照收。
fn moves(params: &Value) -> bool {
    params
        .get("text")
        .and_then(Value::as_str)
        .and_then(|text| text.trim_start().strip_prefix("/workspace"))
        .is_some_and(|rest| rest.starts_with(char::is_whitespace) && !rest.trim().is_empty())
}

#[cfg(test)]
mod tests;
