//! 脚本调工具的执行器这一头（施工 M-2，设计 `39-脚本调工具.md` 第四节，蓝图 `kernel/session.md`「嵌套调用」）：
//!
//! - 端口：每一次直接调的 `shell` 造一个（`port.rs`），交给 [`miyu_tool::Call::nested`]；通道（施工 M-3）每收一个请求调它
//!   一次。端口在父级下面从 1 编号，送进收件箱，等内核答；命令放到后台以后挂到那个任务下面。
//! - 跑：照直接调用的 `run`，但没有任务、子代理、留言、列会话、用量、提问、目标、软件包的端口，不开通道；记忆、知识库、
//!   设置照这一轮的。跑完的结果先存成 blob（交给脚本的那一份：有 `data` 的是它的一行 JSON，没有的是字），再送回去。
//! - 嵌套调用不带效果、不发消息、不结束回合（第四节第 7 条）：报了的记一行 `ERROR`、丢掉，结果照常交回；图片不交给脚本。
//! - 答：内核记过、落了盘才出 `Answer`，这时才把结果交给等着的那一头。等着的那一头没了（脚本被杀）的，不理。

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::oneshot;
use tokio::task::AbortHandle;
use tracing::Instrument;

use miyu_kernel::block::Block;
use miyu_kernel::event::{NestedStatus, Permission};
use miyu_kernel::id::{CallId, ContentHash, JobId, NestedId, Parent, Seq, TurnId};
use miyu_kernel::session::{Input, NestedAnswer, NestedInput};
use miyu_kernel::time::Timestamp;
use miyu_tool::{Call, CallIds, Done, Nested, Progress, Settled, Stop};

use super::{Tools, confine};
use crate::TARGET;
use crate::config::TurnConfig;
use crate::port::Back;

mod port;

/// 执行器手里的嵌套调用。
#[derive(Default)]
pub(crate) struct Nesting {
    /// 送进了内核、还没答的：等着的那一头，派它的那次调用，跑完了的结果。
    waiting: BTreeMap<NestedId, Waiting>,
    /// 在跑的。
    running: BTreeMap<NestedId, Running>,
    /// 记进了日志的后台命令（施工 M-3）：挂在它们下面的才交进内核。
    started: BTreeSet<JobId>,
    /// 拿了端口、结果还没记进日志的 `shell` 调用（`port.rs`）。
    ports: BTreeSet<CallId>,
    /// 拿了端口、结果已经记进日志的：挂在它起的后台任务下面、那个任务又没报的，再也等不到。
    closed: BTreeSet<CallId>,
    /// 挂在还没记进日志的后台命令下面的，等着（`port.rs` 的 `note_jobs`）。
    held: Vec<NestedBack>,
}

struct Waiting {
    reply: oneshot::Sender<Nested>,
    /// 派它的那次直接调用：后台任务下面的是起这个任务的那次（施工 M-3）。工具要的调用编号、记忆的出处照它。
    origin: CallId,
    /// 跑完了：交给脚本的那一份，等答的时候交。
    result: Option<Settled>,
    /// 权限策略判过的真实位置（施工 5-18）：判的时候记下，跑的时候交给工具。
    checked: Option<Arc<[PathBuf]>>,
}

struct Running {
    task: AbortHandle,
    stop: Stop,
    started: Instant,
}

/// 端口和跑它的任务送回 actor 的。
#[derive(Debug)]
pub(crate) enum NestedBack {
    /// 脚本里来了一次。
    Call {
        id: NestedId,
        origin: CallId,
        name: String,
        args: String,
        reply: oneshot::Sender<Nested>,
    },
    /// 跑完了：交给脚本的那一份，出没出错，存成的 blob 和字节数（存不下来的没有）。
    Done {
        id: NestedId,
        settled: Settled,
        error: bool,
        stored: Option<(ContentHash, u64)>,
    },
}

/// 派一次嵌套调用要的：内核的 `Run` 里的几样，加上这一轮的配置。
pub(crate) struct NestedRun {
    pub(crate) id: NestedId,
    pub(crate) name: String,
    pub(crate) args: String,
    pub(crate) cwd: String,
    pub(crate) dirs: Vec<String>,
    pub(crate) permission: Permission,
    /// 这一轮（后台任务下面的、回合外的没有）：记忆的出处。
    pub(crate) turn: Option<TurnId>,
    /// 还算数的最近一次压缩到哪：搜以前的对话跳过这以后的。
    pub(crate) compacted: Option<Seq>,
    pub(crate) config: TurnConfig,
}

impl Tools {
    /// 端口和跑它的任务送回来的，写成内核的输入。不在跑的（掐掉了的）跑完了不理。
    pub(crate) fn nested_back(&mut self, at: Timestamp, back: NestedBack) -> Option<Input> {
        match back {
            NestedBack::Call {
                id,
                origin,
                name,
                args,
                reply,
            } => {
                // 后台任务下面的，等那个任务的 `job.started` 记进日志再交（`port.rs`）：不然内核认不出这个父级。
                if let Parent::Job(job) = id.parent()
                    && !self.nested.started.contains(job)
                {
                    // 派它的那次 `shell` 的结果已经记了、却没报这个任务：再也等不到，当场答「父级不在了」。
                    if self.nested.closed.contains(&origin) {
                        port::gone(reply);
                        return None;
                    }
                    let held = NestedBack::Call {
                        id,
                        origin,
                        name,
                        args,
                        reply,
                    };
                    self.nested.held.push(held);
                    return None;
                }
                let waiting = Waiting {
                    reply,
                    origin,
                    result: None,
                    checked: None,
                };
                self.nested.waiting.insert(id.clone(), waiting);
                Some(Input::Nested(NestedInput::Call { at, id, name, args }))
            }
            NestedBack::Done {
                id,
                settled,
                error,
                stored,
            } => {
                let running = self.nested.running.remove(&id)?;
                let (result, bytes) = stored.unzip();
                if let Some(waiting) = self.nested.waiting.get_mut(&id) {
                    waiting.result = Some(settled);
                }
                Some(Input::Nested(NestedInput::Done {
                    at,
                    id,
                    error,
                    duration_ms: Some(crate::lines::millis(running.started.elapsed())),
                    result,
                    bytes,
                }))
            }
        }
    }

    /// 记下嵌套调用 `id` 判过的真实位置 `checked`（施工 5-18）：不在等着的（掐掉了的）不记。
    pub(crate) fn checked_nested(&mut self, id: &NestedId, checked: Vec<PathBuf>) {
        if let Some(waiting) = self.nested.waiting.get_mut(id) {
            waiting.checked = Some(Arc::from(checked));
        }
    }

    /// 跑一次嵌套调用：在自己的任务里跑，马上返回。目录里没有这件工具的，当场照出错交回。
    pub(crate) fn run_nested(&mut self, at: Timestamp, run: NestedRun) -> Option<Input> {
        let NestedRun {
            id,
            name,
            args,
            cwd,
            dirs,
            permission,
            turn,
            compacted,
            config,
        } = run;
        // 内核只派收下了的：收下时一定记了派它的那次调用。没有的是 bug，不跑；父级结束时照「已取消」收掉。判过的真实位置
        // 没记下的（不该有）按一个都没判过算，工具换出来的一律不碰（施工 5-18）。
        let Some((origin, checked)) = self.nested.waiting.get_mut(&id).map(|waiting| {
            let checked = waiting.checked.take();
            (
                waiting.origin,
                checked.unwrap_or_else(|| Arc::from(Vec::new())),
            )
        }) else {
            tracing::error!(target: TARGET, nested = id.to_string().as_str(), "nested call run before it came");
            return None;
        };
        let Some(tool) = self.catalog.get(&name) else {
            let worded = self.lettering.run().unavailable(&name);
            if let Some(waiting) = self.nested.waiting.get_mut(&id) {
                waiting.result = Some(Settled {
                    text: worded.text,
                    ..Settled::without_result(NestedStatus::Error, None)
                });
            }
            return Some(Input::Nested(NestedInput::Done {
                at,
                id,
                error: true,
                duration_ms: None,
                result: None,
                bytes: None,
            }));
        };
        let stop = Stop::default();
        let call = Call {
            args,
            cwd,
            home: self.home.clone(),
            data_root: Some(self.data_root.clone()),
            seen: Arc::clone(&self.seen),
            stop: stop.clone(),
            sandbox: None,
            log: Some(self.log.clone()),
            offset: self.offset,
            agents: None,
            messages: None,
            jobs: None,
            sessions: None,
            usage: None,
            questions: None,
            memory: self
                .memory
                .as_ref()
                .map(|memory| memory.port(turn, compacted, origin, at, &config)),
            knowledge: crate::knowledge::port(self.knowledge.as_ref(), &config),
            packages: None,
            goals: None,
            nested: None,
            ids: Some(CallIds {
                session: self.session.clone(),
                owner: self.owner.clone(),
                call: origin,
                asked: None,
                preset: self.preset.clone(),
            }),
            settings: Some(Arc::clone(&config) as Arc<dyn miyu_tool::SettingsPort>),
            checked: Some(checked),
        };
        let shown = id.to_string();
        tracing::info!(target: TARGET, nested = shown.as_str(), tool = name.as_str(), "running nested");
        let span = tracing::Span::current();
        let wrap = self.wrap(&name, &tool, &call);
        let sandbox = self.sandbox.clone().filter(|_| wrap.needs_spec());
        let unless = wrap.unless();
        let inner = tokio::spawn(
            async move {
                let call = confine(
                    call,
                    sandbox,
                    (permission, dirs, Vec::new()),
                    (origin, unless),
                )
                .await?;
                Some(tool.run(call, Progress::new(|_| {})).await)
            }
            .instrument(span.clone()),
        );
        let task = inner.abort_handle();
        let backs = self.backs.clone();
        let blobs = self.blobs.clone();
        let crashed = self.lettering.run().crashed(&name).text;
        let watched = id.clone();
        tokio::spawn(
            async move {
                let (settled, error) = match inner.await {
                    Ok(Some(done)) => settle(&watched, done),
                    Ok(None) => (crashed_result(crashed), true),
                    Err(error) if error.is_panic() => (crashed_result(crashed), true),
                    // 掐掉了：没人要了。
                    Err(_) => return,
                };
                let output = output(&settled);
                let stored = tokio::task::spawn_blocking(move || {
                    span.in_scope(|| {
                        blobs
                            .put(output.as_bytes())
                            .ok()
                            .map(|hash| (hash, output.len() as u64))
                    })
                })
                .await
                .ok()
                .flatten();
                let back = NestedBack::Done {
                    id: watched,
                    settled,
                    error,
                    stored,
                };
                #[expect(
                    clippy::let_underscore_must_use,
                    reason = "会话停了：结果没人要了，丢掉"
                )]
                let _ = backs.send(Back::Nested(back));
            }
            .instrument(tracing::Span::current()),
        );
        self.nested.running.insert(
            id,
            Running {
                task,
                stop,
                started: Instant::now(),
            },
        );
        None
    }

    /// 掐掉在跑的那一个：举旗、掐任务；之后到的不理。
    pub(crate) fn cancel_nested(&mut self, id: &NestedId) {
        if let Some(running) = self.nested.running.remove(id) {
            running.stop.raise();
            running.task.abort();
        }
    }

    /// 记过、落了盘：答等着的那一头。跑完的交结果，别的只交结局。
    pub(crate) fn answer_nested(&mut self, id: &NestedId, answer: NestedAnswer) {
        let Some(waiting) = self.nested.waiting.remove(id) else {
            return;
        };
        let nested = match answer {
            NestedAnswer::Refused(refusal) => Nested::Refused(refusal),
            NestedAnswer::Settled { status, reason } => Nested::Settled(match waiting.result {
                Some(result) if matches!(status, NestedStatus::Ok | NestedStatus::Error) => {
                    Settled { status, ..result }
                }
                _ => Settled::without_result(status, reason),
            }),
        };
        #[expect(
            clippy::let_underscore_must_use,
            reason = "脚本那头不等了（被杀了）：答了没人听"
        )]
        let _ = waiting.reply.send(nested);
    }
}

/// 会话停了：在跑的都掐掉，等着的那一头跟着读到「没跑完」。
impl Drop for Nesting {
    fn drop(&mut self) {
        for running in self.running.values() {
            running.stop.raise();
            running.task.abort();
        }
    }
}

/// 工具交回的写成交给脚本的那一份。效果、发出去的、结束回合不认：报了的是开错了，记一行 `ERROR`、丢掉。
fn settle(id: &NestedId, done: Done) -> (Settled, bool) {
    if !done.effects.is_empty() || !done.sent.is_empty() || done.end_turn {
        tracing::error!(
            target: TARGET,
            nested = id.to_string().as_str(),
            effects = done.effects.len(),
            sent = done.sent.len(),
            end_turn = done.end_turn,
            "a nested call reported effects, sent messages or ended the turn; dropped"
        );
    }
    let text = done
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let status = match done.error {
        true => NestedStatus::Error,
        false => NestedStatus::Ok,
    };
    let settled = Settled {
        status,
        reason: None,
        text,
        data: done.data.filter(|_| !done.error),
        images_dropped: !done.images.is_empty(),
    };
    (settled, done.error)
}

/// 崩了的：照直接调用崩了的那一句。
fn crashed_result(text: String) -> Settled {
    Settled {
        text,
        ..Settled::without_result(NestedStatus::Error, None)
    }
}

/// 存进 blob、给人点开看的那一份：有结构化结果的是它的一行 JSON，没有的是字。
fn output(settled: &Settled) -> String {
    match &settled.data {
        Some(data) => data.to_string(),
        None => settled.text.clone(),
    }
}
