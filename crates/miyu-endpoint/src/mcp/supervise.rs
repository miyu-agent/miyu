//! 看管一个服务（施工 X-3 上，`mcp.md`「怎么走」第二条第 4、6 条，第三条第 2 条）：起进程，认时代、列工具、登记，等它退出或者
//! 请它停；服务说工具变了的再列一次。退出了照扩展的办法退避着重启，连续 5 次停下；认不出时代的直接停下。请它停就丢下客户端
//! （关它的标准输入），等一会儿没退的杀掉。

use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio::process::Child;
use tokio::sync::watch;
use tokio::time::Instant;

use miyu_mcp::{Client, Failed, Hello, Waits};

use super::launch::{self, Plan};
use super::{Shared, TARGET, spec};
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
    loop {
        let ended = once(&core, &plan, &shared, &mut asked, &timing, failures).await;
        let Some(owner) = core.upgrade() else {
            return;
        };
        let ready_at = match ended {
            Ended::Asked | Ended::Gone => return,
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
) -> Ended {
    let service = plan.server.name.as_str();
    let mut child = match launch::spawn(plan).await {
        Ok(child) => child,
        Err(error) => {
            tracing::warn!(target: TARGET, service, program = %plan.program.display(), error = %error, "mcp not started");
            return Ended::Failed(None);
        }
    };
    let pid = child.id().unwrap_or_default();
    tracing::info!(target: TARGET, service, pid, "mcp started");
    {
        let Some(owner) = core.upgrade() else {
            return Ended::Gone;
        };
        shared.set(&owner, State::Starting { pid: Some(pid) }, failures);
    }
    let (Some(input), Some(output)) = (child.stdin.take(), child.stdout.take()) else {
        // 两头都接了管道，不会没有。
        return Ended::Stop(Reason::CannotStart);
    };
    let connecting = Client::connect(output, input, hello(), None, Waits::default());
    let client = tokio::select! {
        connected = connecting => connected,
        _ = asked.changed() => {
            finish(&mut child, timing.grace, service).await;
            return Ended::Asked;
        }
    };
    let client = match client {
        Ok(client) => Arc::new(client),
        Err(error) => {
            tracing::warn!(target: TARGET, service, error = %error, "mcp not connected");
            finish(&mut child, timing.grace, service).await;
            return match error {
                Failed::NoCommonVersion(_) => Ended::Stop(Reason::CannotStart),
                _ => Ended::Failed(None),
            };
        }
    };
    let mut changes = client.changes();
    if !list(core, plan, shared, &client).await {
        drop(client);
        finish(&mut child, timing.grace, service).await;
        return Ended::Failed(None);
    }
    let ready_at = Instant::now();
    {
        let Some(owner) = core.upgrade() else {
            return Ended::Gone;
        };
        shared.connect(Some(Arc::clone(&client)));
        tracing::info!(target: TARGET, service, pid, era = ?client.era(), "mcp ready");
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
        }
    };
    shared.connect(None);
    drop(client);
    let status = finish(&mut child, timing.grace, service).await;
    if stopping {
        tracing::info!(target: TARGET, service, status = status.as_str(), "mcp exited");
        return Ended::Asked;
    }
    tracing::warn!(target: TARGET, service, status = status.as_str(), "mcp exited");
    Ended::Failed(Some(ready_at))
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
    for problem in &listed.skipped {
        tracing::warn!(target: TARGET, service, problem = problem.as_str(), "mcp tool skipped");
    }
    let Some(owner) = core.upgrade() else {
        return false;
    };
    spec::register(&owner, &shared.package, &plan.server, &listed.tools, shared);
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
