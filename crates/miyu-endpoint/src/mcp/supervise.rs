//! 看管一个服务（施工 X-3 上，`mcp.md`「怎么走」第二条第 4、6 条，第三条第 2 条）：起进程，认时代、列工具、登记，等它退出或者
//! 请它停；服务说工具变了的再列一次。退出了照扩展的办法退避着重启，连续 5 次停下；认不出时代的直接停下。请它停就丢下客户端
//! （关它的标准输入），等一会儿没退的杀掉。

use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio::process::Child;
use tokio::sync::watch;
use tokio::time::Instant;

use miyu_mcp::{Client, Failed, Hello, Waits};

use super::launch::{self, How, Plan};
use super::{Shared, Skipped, TARGET, cache, spec};
use crate::Core;
use crate::extensions::{Reason, State, Timing};

/// 连续失败几次停下：同扩展。
const TRIES: u32 = 5;

/// 起了一次的结局。
enum Ended {
    /// 请它停了：已经停下。
    Asked,
    /// 核心没了。
    Gone,
    /// 不再起：原因。
    Stop(Reason),
    /// 失败了一次：握成了的有握成的那一刻。
    Failed(Option<Instant>),
    /// 网址的服务跑起来以后会话过期了（施工 X-32 上）：不算失败、不退避，马上重连、从认时代重来。没跑起来就过期的照
    /// [`Ended::Failed`] 算，一直回 `404` 的服务不会原地打转。
    Expired,
}

/// 看管服务 `plan.server`，直到停下，或者 `asked` 来了（请它退出、等它退出以后交回，状态由叫停的一方记）。核心没了的也交回。
pub(super) async fn run(
    core: Weak<Core>,
    plan: Plan,
    shared: Arc<Shared>,
    mut asked: watch::Receiver<bool>,
    timing: Timing,
) {
    let mut failures = 0;
    // 照哪个时代说：先照缓存记着的，连上一次以后照认出来的（施工 X-3 下）。
    let mut known = plan.known.clone();
    loop {
        let ended = once(
            &core, &plan, &shared, &mut asked, &timing, failures, &mut known,
        )
        .await;
        let Some(owner) = core.upgrade() else {
            return;
        };
        let ready_at = match ended {
            Ended::Asked | Ended::Gone => return,
            Ended::Expired => {
                tracing::info!(target: TARGET, service = plan.server.name.as_str(), "mcp session expired");
                continue;
            }
            Ended::Stop(reason) => {
                tracing::warn!(target: TARGET, service = plan.server.name.as_str(), reason = reason.as_str(), "mcp stopped");
                shared.set(&owner, State::Stopped(reason), failures);
                return;
            }
            Ended::Failed(ready_at) => ready_at,
        };
        let stable = ready_at.is_some_and(|at| at.elapsed() >= timing.stable);
        failures = if stable { 1 } else { failures + 1 };
        if failures >= TRIES {
            tracing::warn!(target: TARGET, service = plan.server.name.as_str(), failures, "mcp stopped");
            shared.set(&owner, State::Stopped(Reason::FailedRepeatedly), failures);
            return;
        }
        let after = crate::extensions::backoff(&timing, failures);
        tracing::info!(target: TARGET, service = plan.server.name.as_str(), failures, after_ms = after.as_millis(), "mcp waiting");
        shared.set(
            &owner,
            State::Waiting {
                until: Instant::now() + after,
            },
            failures,
        );
        drop(owner);
        tokio::select! {
            () = tokio::time::sleep(after) => {}
            _ = asked.changed() => return,
        }
    }
}

/// 起一次：起进程、认时代、列工具、登记，等它退出或者请它停。
async fn once(
    core: &Weak<Core>,
    plan: &Plan,
    shared: &Arc<Shared>,
    asked: &mut watch::Receiver<bool>,
    timing: &Timing,
    failures: u32,
    known: &mut Option<miyu_mcp::Era>,
) -> Ended {
    let service = plan.server.name.as_str();
    let (mut running, pid, output, input) = match start(plan).await {
        Ok(started) => started,
        Err(ended) => return ended,
    };
    tracing::info!(target: TARGET, service, pid = ?pid, "mcp started");
    {
        let Some(owner) = core.upgrade() else {
            return Ended::Gone;
        };
        shared.set(&owner, State::Starting { pid }, failures);
    }
    let connecting = Client::connect(output, input, hello(), known.clone(), Waits::default());
    let client = tokio::select! {
        connected = connecting => connected,
        _ = asked.changed() => {
            running.finish(timing.grace, service).await;
            return Ended::Asked;
        }
    };
    let client = match client {
        Ok(client) => Arc::new(client),
        Err(error) => {
            tracing::warn!(target: TARGET, service, error = %error, "mcp not connected");
            running.finish(timing.grace, service).await;
            return match error {
                Failed::NoCommonVersion(_) => Ended::Stop(Reason::CannotStart),
                _ => Ended::Failed(None),
            };
        }
    };
    *known = Some(client.era().clone());
    let mut changes = client.changes();
    if !list(core, plan, shared, &client).await {
        drop(client);
        running.finish(timing.grace, service).await;
        return Ended::Failed(None);
    }
    let ready_at = Instant::now();
    {
        let Some(owner) = core.upgrade() else {
            return Ended::Gone;
        };
        shared.connect(Some(Arc::clone(&client)));
        tracing::info!(target: TARGET, service, pid = ?pid, era = ?client.era(), "mcp ready");
        shared.set(&owner, State::Running { pid }, failures);
    }
    let stopping = loop {
        tokio::select! {
            () = client.closed() => break false,
            _ = asked.changed() => break true,
            changed = changes.changed() => {
                if changed.is_err() || !list(core, plan, shared, &client).await {
                    break false;
                }
            }
            // 目录过期以后有人要用它（施工 X-3 下，`ttlMs`）。
            () = shared.relist.notified() => {
                if !list(core, plan, shared, &client).await {
                    break false;
                }
            }
        }
    };
    shared.connect(None);
    drop(client);
    let status = running.finish(timing.grace, service).await;
    if stopping {
        tracing::info!(target: TARGET, service, status = status.as_str(), "mcp exited");
        return Ended::Asked;
    }
    if running.expired() {
        return Ended::Expired;
    }
    tracing::warn!(target: TARGET, service, status = status.as_str(), "mcp exited");
    Ended::Failed(Some(ready_at))
}

/// 起来的那一头：进程，或者网址的服务（没有进程：丢下客户端，转接跟着停）。进程那一种装进盒子：Windows 上 `Child` 比过期的
/// 旗大得多，clippy 的 `large_enum_variant` 只在 Windows 上报。
enum Running {
    Child(Box<Child>),
    Remote(miyu_mcp::http::Expired),
}

impl Running {
    /// 收尾：进程照 [`finish`]；网址的没什么要等。交回怎么结束的，写成字。
    async fn finish(&mut self, grace: Duration, service: &str) -> String {
        match self {
            Running::Child(child) => finish(child, grace, service).await,
            Running::Remote(_) => "closed".to_string(),
        }
    }

    /// 网址的服务这条连接是会话过期断的。
    fn expired(&self) -> bool {
        matches!(self, Running::Remote(expired) if expired.happened())
    }
}

/// 读的一头，装箱以后两种起法一样交给 [`Client::connect`]。
type Reader = Box<dyn tokio::io::AsyncRead + Unpin + Send>;

/// 写的一头，同 [`Reader`]。
type Writer = Box<dyn tokio::io::AsyncWrite + Unpin + Send>;

/// 起一次：拉起进程，或者接上网址（施工 X-32 上）。交回在跑的那一头、进程号（网址的没有）、读写两头；起不来的交回结局。
async fn start(plan: &Plan) -> Result<(Running, Option<u32>, Reader, Writer), Ended> {
    let service = plan.server.name.as_str();
    match &plan.how {
        How::Spawn { program, .. } => {
            let mut child = launch::spawn(plan).await.map_err(|error| {
                tracing::warn!(target: TARGET, service, program = %program.display(), error = %error, "mcp not started");
                Ended::Failed(None)
            })?;
            let pid = child.id();
            let (Some(input), Some(output)) = (child.stdin.take(), child.stdout.take()) else {
                // 两头都接了管道，不会没有。
                return Err(Ended::Stop(Reason::CannotStart));
            };
            Ok((
                Running::Child(Box::new(child)),
                pid,
                Box::new(output),
                Box::new(input),
            ))
        }
        How::Connect { url, headers } => {
            let log = log_to(plan.log.clone());
            let local = miyu_http::is_loopback_url(url);
            // 令牌不明文发到别的机器上（`mcp.md`「网址的服务怎么说话」）：`http://` 只给本机地址。
            if url.starts_with("http://") && !local {
                log("http:// is only for this machine, use https://".to_string());
                tracing::warn!(target: TARGET, service, "mcp url not https");
                return Err(Ended::Stop(Reason::CannotStart));
            }
            let proxy = if local {
                miyu_http::Proxy::Off
            } else {
                miyu_http::Proxy::FromEnvironment
            };
            let client = miyu_http::client(proxy).map_err(|error| {
                tracing::warn!(target: TARGET, service, error = %error, "mcp not started");
                Ended::Failed(None)
            })?;
            let opened = miyu_mcp::http::open(client, url.clone(), headers.clone(), log);
            Ok((
                Running::Remote(opened.expired),
                None,
                Box::new(opened.read),
                Box::new(opened.write),
            ))
        }
    }
}

/// 往服务的日志（`state/logs/mcp-<服务名>.stderr`）追加一行：网址的服务没有标准错误，对方回错这一类记在这里，软件包列表上停下了的
/// 照旧给最后几行。写不进的不管：这只是给人看的线索。
fn log_to(path: std::path::PathBuf) -> miyu_mcp::http::Log {
    std::sync::Arc::new(move |line: String| {
        if let Ok(mut file) = crate::extensions::stderr::open(&path) {
            use std::io::Write;
            if writeln!(file, "{line}").is_err() {
                // 写不进日志：线索丢了，服务照常。
            }
        }
    })
}

/// 列工具、登记：交回成没成。核心没了的算没成。
async fn list(core: &Weak<Core>, plan: &Plan, shared: &Arc<Shared>, client: &Client) -> bool {
    let service = plan.server.name.as_str();
    let listed = match client.tools().await {
        Ok(listed) => listed,
        Err(error) => {
            tracing::warn!(target: TARGET, service, error = %error, "mcp tools not listed");
            return false;
        }
    };
    // 认不出来的、登记时查不过的，记进看管那一格（施工 X-37）。
    let mut skipped: Vec<Skipped> = listed.skipped.iter().map(Skipped::unread).collect();
    for problem in &listed.skipped {
        tracing::warn!(target: TARGET, service, problem = %problem, "mcp tool skipped");
    }
    let Some(owner) = core.upgrade() else {
        return false;
    };
    skipped.extend(spec::register(
        &owner,
        &shared.package,
        &plan.server,
        &listed.tools,
        shared,
    ));
    // 照原名排：每次列完说的一样。
    skipped.sort();
    shared.set_skipped(skipped);
    shared.listed(listed.fresh_for);
    cache::write(&owner, &plan.server, client.era(), &listed.tools);
    true
}

/// 我们是谁：`miyu` 和版本。
fn hello() -> Hello {
    Hello {
        name: "miyu".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

/// 客户端丢下了（标准输入关了）：等它退出，最多等 `grace`，没退的杀掉。交回它怎么退出的，写成字。
async fn finish(child: &mut Child, grace: Duration, service: &str) -> String {
    let waited = match tokio::time::timeout(grace, child.wait()).await {
        Ok(waited) => waited.ok(),
        Err(_) => {
            if let Err(error) = child.start_kill() {
                tracing::warn!(target: TARGET, service, error = %error, "mcp not killed");
            }
            child.wait().await.ok()
        }
    };
    waited.map_or_else(|| "unknown".to_string(), |status| status.to_string())
}
