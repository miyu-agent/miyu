//! 通道两个平台共用的（施工 M-3）：一问一答的协议、令牌、认人等的那一会儿、环境变量、端点跟着进程交给任务表。端点本身
//! （Unix 套接字、Windows 命名管道）和怎么认连上来的进程在 `unix.rs`、`windows.rs`。

use std::ffi::OsString;
use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use serde::Deserialize;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use miyu_tool::{Exit, NestedPort, Process};

use super::Texts;
use super::reply::{self, Reply, USAGE};
use crate::load::say;
use crate::shell::process::Member;

/// 告诉 `miyu call` 端点在哪的环境变量：Unix 上是套接字的路径，Windows 上是管道的名字。
pub(super) const SOCK_VAR: &str = "MIYU_CALL_SOCK";

/// 令牌的环境变量。
pub(super) const TOKEN_VAR: &str = "MIYU_CALL_TOKEN";

/// 协议版本：请求带它，对不上的回一句、退出码 2。
pub(super) const VERSION: u32 = 1;

/// 请求帧的上限。
pub(super) const MAX_REQUEST: u32 = 1 << 20;

/// 同一个端点同时连着的上限。
pub(super) const MAX_OPEN: usize = 32;

/// 命令刚起、还没记下它是谁时，连上来的最多等多久。
const BIND_WAIT: Duration = Duration::from_secs(5);

/// 运行日志的来处，和 `shell` 的别处一样。
pub(super) const TARGET: &str = "miyu::shell";

/// 这条命令是谁（进程组、作业对象）：起来以后才知道，记一次。
pub(super) type Bound = Arc<OnceLock<Member>>;

/// 记这条命令是谁的那一头。
pub(in crate::shell) struct Binder(pub(super) Bound);

impl Binder {
    /// 记下 `member`：只记一次。
    pub(in crate::shell) fn bind(self, member: Member) {
        if self.0.set(member).is_err() {
            tracing::debug!(target: TARGET, "miyu call endpoint bound twice");
        }
    }
}

/// 等到记下了这条命令是谁，最多等 [`BIND_WAIT`]；等不到的没有。
#[cfg_attr(
    unix,
    expect(
        clippy::clone_on_copy,
        reason = "Unix 上是进程组号，Windows 上是作业对象，那边要 clone"
    )
)]
pub(super) async fn bound(member: &OnceLock<Member>) -> Option<Member> {
    tokio::time::timeout(BIND_WAIT, async {
        loop {
            if let Some(member) = member.get() {
                return member.clone();
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .ok()
}

/// 32 个十六进制字符的随机串：128 位。
///
/// # Errors
///
/// 系统给不出随机数。
pub(super) fn random_hex() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(io::Error::other)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// 环境变量：端点、令牌，`PATH` 最前面加上放 `miyu` 的目录 `bin`（照这个系统的写法连）。加在挑好的那几个后面。
pub(super) fn env(
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

/// 是不是 `PATH`：Windows 上名字不分大小写。
fn is_path(name: &OsString) -> bool {
    match cfg!(windows) {
        true => name.to_string_lossy().eq_ignore_ascii_case("PATH"),
        false => name == "PATH",
    }
}

/// 放到后台的命令带着它的端点 `channel` 交给任务表：任务结束（进程收掉）时一起关。
pub(super) fn keep<C: Send + Sync + 'static>(
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
pub(super) async fn answer<S, F>(
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
        Ok(request) if request.token != token || !ours().await => refused(&texts.stranger),
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
