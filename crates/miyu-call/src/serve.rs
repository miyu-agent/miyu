//! 通道两个平台共用的（施工 M-3）：一问一答的协议、令牌、环境变量、交给扩展的那一格、端点跟着进程交给任务表。端点本身
//! （Unix 套接字、Windows 命名管道）在 `unix.rs`、`windows.rs`，认连上来的进程在 `kin.rs`。

use std::ffi::OsString;
use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use miyu_tool::{Exit, NestedPort, Process};

use miyu_tool::load::say;

use crate::reply::{self, Reply, USAGE};
use crate::{TARGET, Texts};

/// 告诉 `miyu call` 端点在哪的环境变量：Unix 上是套接字的路径，Windows 上是管道的名字。
pub(crate) const SOCK_VAR: &str = "MIYU_CALL_SOCK";

/// 令牌的环境变量。
pub(crate) const TOKEN_VAR: &str = "MIYU_CALL_TOKEN";

/// 协议版本：请求带它，对不上的回一句、退出码 2。
pub(crate) const VERSION: u32 = 1;

/// 请求帧的上限。
pub(crate) const MAX_REQUEST: u32 = 1 << 20;

/// 同一个端点同时连着的上限。
pub(crate) const MAX_OPEN: usize = 32;

/// 32 个十六进制字符的随机串：128 位。
///
/// # Errors
///
/// 系统给不出随机数。
pub(crate) fn random_hex() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(io::Error::other)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// 端点名里的那一段（施工 XG-1补2）：另取一个随机串的前 16 个字符，不用令牌的。Windows 的命名管道名全机的进程都列得出来，
/// 用令牌的前一半就等于把半个令牌摆出来。
///
/// # Errors
///
/// 系统给不出随机数。
pub(crate) fn endpoint_tag() -> io::Result<String> {
    let mut tag = random_hex()?;
    tag.truncate(16);
    Ok(tag)
}

/// 令牌对不对（施工 XG-1补2）：等时比较，每个字节都比完再说，不在第一个不同的字节就停：那样的话，量回答多快就能一个字节一个
/// 字节地猜。长度不同的直接不对：令牌的长度本来是公开的（32 个十六进制字符）。
pub(crate) fn same_token(given: &str, token: &str) -> bool {
    let (given, token) = (given.as_bytes(), token.as_bytes());
    if given.len() != token.len() {
        return false;
    }
    let differ = given
        .iter()
        .zip(token)
        .fold(0u8, |differ, (a, b)| differ | (a ^ b));
    std::hint::black_box(differ) == 0
}

/// 环境变量：端点、令牌，`PATH` 最前面加上放 `miyu` 的目录 `bin`（照这个系统的写法连）。加在挑好的那几个后面。
pub(crate) fn env(
    env: &mut Vec<(OsString, OsString)>,
    endpoint: OsString,
    token: &str,
    bin: Option<PathBuf>,
) {
    env.push((SOCK_VAR.into(), endpoint));
    env.push((TOKEN_VAR.into(), token.into()));
    let Some(bin) = bin else {
        return;
    };
    let at = env.iter().position(|(name, _)| is_path(name));
    let rest = at.map(|at| env[at].1.clone()).unwrap_or_default();
    let joined = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&rest)))
        .unwrap_or_else(|_| rest.clone());
    match at {
        Some(at) => env[at].1 = joined,
        None => env.push(("PATH".into(), joined)),
    }
}

/// 交给扩展的那一格（`tool.call` 的 `channel`，施工 XG-1 下）：扩展把 `sock`、`token` 设成 `MIYU_CALL_SOCK`、`MIYU_CALL_TOKEN`
/// 进它起的命令，`bin` 放 `PATH` 最前面；`bin` 建不成的不写。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Handed {
    /// 端点：Unix 上是套接字的路径，Windows 上是管道的名字。
    pub sock: String,
    /// 令牌。
    pub token: String,
    /// 放 `miyu` 的目录。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin: Option<String>,
}

/// 交给扩展的那一格：端点 `endpoint`、令牌 `token`、放 `miyu` 的目录 `bin`。端点不是 UTF-8 的交不出去（JSON 里写不了），没有；
/// `bin` 不是的不写。
pub(crate) fn handed(endpoint: OsString, token: &str, bin: Option<PathBuf>) -> Option<Handed> {
    Some(Handed {
        sock: endpoint.into_string().ok()?,
        token: token.to_string(),
        bin: bin.and_then(|bin| bin.into_os_string().into_string().ok()),
    })
}

/// 是不是 `PATH`：Windows 上名字不分大小写。
fn is_path(name: &OsString) -> bool {
    match cfg!(windows) {
        true => name.to_string_lossy().eq_ignore_ascii_case("PATH"),
        false => name == "PATH",
    }
}

/// 放到后台的命令带着它的端点 `channel` 交给任务表：任务结束（进程收掉）时一起关。
pub(crate) fn keep<C: Send + Sync + 'static>(
    process: Box<dyn Process>,
    channel: C,
) -> Box<dyn Process> {
    Box::new(Kept {
        process,
        _channel: channel,
    })
}

struct Kept<C> {
    process: Box<dyn Process>,
    _channel: C,
}

impl<C: Send + Sync> Process for Kept<C> {
    fn wait(&self) -> io::Result<Exit> {
        self.process.wait()
    }

    fn kill(&self) {
        self.process.kill();
    }
}

/// 一次请求：协议版本、做什么（`call`、`schema`，会话里的 `miyu pkg` 是 `pkg`，施工 BS-1 中）、工具名、参数原文、令牌。
#[derive(Deserialize)]
struct Request {
    v: u32,
    op: String,
    tool: String,
    #[serde(default)]
    args: String,
    #[serde(default)]
    token: String,
}

/// 在连上来的流上答一个请求：读一帧，令牌对得上、`ours` 认得出这个进程才调，答一帧。读写出错就丢掉：对方已经不在了。
pub(crate) async fn answer<S, F>(
    mut stream: S,
    ours: impl FnOnce() -> F,
    port: Arc<dyn NestedPort>,
    texts: Arc<Texts>,
    token: String,
) where
    S: AsyncRead + AsyncWrite + Unpin,
    F: Future<Output = bool>,
{
    let Ok(len) = stream.read_u32().await else {
        return;
    };
    // 超过上限的只看长度就断开，不等后面的字：等的话，对方报个大长度就能把这条连接一直占着。
    if len > MAX_REQUEST {
        return;
    }
    let mut body = vec![0u8; len as usize];
    if stream.read_exact(&mut body).await.is_err() {
        return;
    }
    let refused = |template| Reply::said(USAGE, say(template, &[]));
    let reply = match serde_json::from_slice::<Request>(&body) {
        Err(_) => refused(&texts.bad_request),
        Ok(request) if !same_token(&request.token, &token) || !ours().await => {
            refused(&texts.stranger)
        }
        Ok(request) if request.v != VERSION => refused(&texts.version),
        Ok(request) => match request.op.as_str() {
            "call" => reply::call(&*port, &texts, request.tool, request.args).await,
            "pkg" => reply::command(&*port, &texts, request.tool, request.args).await,
            "schema" => reply::schema(&*port, &texts, &request.tool),
            _ => refused(&texts.bad_request),
        },
    };
    let body = serde_json::json!({
        "code": reply.code,
        "stdout": reply.stdout,
        "stderr": reply.stderr,
    })
    .to_string();
    if let Err(error) = send(&mut stream, &body).await {
        tracing::debug!(target: TARGET, error = %error, "miyu call reply not sent");
    }
}

/// 写一帧：4 字节大端长度加字。
async fn send<S: AsyncWrite + Unpin>(stream: &mut S, body: &str) -> io::Result<()> {
    let len = u32::try_from(body.len()).map_err(io::Error::other)?;
    stream.write_u32(len).await?;
    stream.write_all(body.as_bytes()).await?;
    stream.flush().await
}

#[cfg(test)]
mod tests;
