//! 脚本调工具的端口（施工 M-2、M-3）：每一次直接调的 `shell` 造一个，挂在这次调用下面；命令放到后台（一起来就放的、到时
//! 挪过去的）以后，之后的调用挂到那个任务下面（[`NestedPort::adopt`]）。端口在父级下面从 1 编号，一次调用送进收件箱，等内核答。
//!
//! 挂在后台任务下面的，等那个任务的 `job.started` 记进日志再交进内核：任务是 `shell` 的结果报的，结果落下之前内核认不出
//! 这个父级，会当它不在了。actor 每记一批事件看一遍（[`Tools::note_jobs`]），记进了的把等着的重新送回收件箱。
//!
//! 说明和参数格式照这个会话的工具面（内核的规则里 `clicall` 开着的、访问类别准的那几件），说明照工具目录。

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::{mpsc, oneshot};

use miyu_kernel::event::{Body, Effect, Event, JobKind, NestedStatus};
use miyu_kernel::id::{CallId, JobId, NestedId, Parent};
use miyu_kernel::raw::RawJson;
use miyu_tool::{Calling, Nested, NestedPort, Schema, Settled};

use super::NestedBack;
use crate::port::Back;
use crate::tools::Tools;

/// 挂在一个父级下面的端口。
struct Nester {
    /// 现在挂在谁下面：先是那次调用，放到后台以后是那个任务。
    parent: Mutex<Parent>,
    /// 派它的那次直接调用。
    origin: CallId,
    /// 上一个编到几号：换了父级接着编，编号照样不重。
    last: AtomicU32,
    /// 脚本里能调的：名字到说明、参数格式。
    face: BTreeMap<String, Schema>,
    backs: mpsc::UnboundedSender<Back>,
}

/// 没答就了结的：当没跑完。
fn cancelled() -> Nested {
    Nested::Settled(Settled::without_result(NestedStatus::Cancelled, None))
}

impl NestedPort for Nester {
    fn call(&self, name: String, args: String) -> Calling<'_> {
        Box::pin(async move {
            let n = self.last.fetch_add(1, Ordering::AcqRel) + 1;
            let parent = self
                .parent
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone();
            let Some(id) = NestedId::new(parent, n) else {
                return cancelled();
            };
            let (reply, answer) = oneshot::channel();
            let back = NestedBack::Call {
                id,
                origin: self.origin,
                name,
                args,
                reply,
            };
            if self.backs.send(Back::Nested(back)).is_err() {
                return cancelled();
            }
            // 会话停了、没答就丢了。
            answer.await.unwrap_or_else(|_| cancelled())
        })
    }

    fn adopt(&self, job: JobId) {
        *self.parent.lock().unwrap_or_else(PoisonError::into_inner) = Parent::Job(job);
    }

    fn callable(&self) -> Vec<String> {
        self.face.keys().cloned().collect()
    }

    fn schema(&self, name: &str) -> Option<Schema> {
        self.face.get(name).cloned()
    }
}

impl Tools {
    /// 直接调的 `shell` 第 `call_id` 次调用的端口：挂在这次调用下面。`clicall` 是这个会话脚本里能调的工具和参数格式（内核的
    /// `Session::clicall_tools`），说明照工具目录；目录里没有的不列。
    pub(crate) fn nested_port(
        &mut self,
        call_id: CallId,
        clicall: Vec<(String, RawJson)>,
    ) -> Arc<dyn NestedPort> {
        let face = clicall
            .into_iter()
            .filter_map(|(name, parameters)| {
                let tool = self.catalog.get(&name)?;
                let schema = Schema {
                    description: tool.spec().description.clone(),
                    parameters: parameters.get().to_string(),
                };
                Some((name, schema))
            })
            .collect();
        self.nested.ports.insert(call_id);
        Arc::new(Nester {
            parent: Mutex::new(Parent::Call(call_id)),
            origin: call_id,
            last: AtomicU32::new(0),
            face,
            backs: self.backs.clone(),
        })
    }

    /// 记了一批事件：报了开始的后台命令记下，挂在它们下面、等着的重新送回收件箱。派它的那次 `shell` 的结果记了、却没报这个
    /// 任务的（调用被打断、任务没交上），等着的再也等不到：当场答「父级不在了」，脚本那头退出码 3，不一直等；之后才到的同样
    /// （`nested.rs` 收下时看 `closed`）。拿了端口的 `shell` 结果记了，记进 `closed`。
    pub(crate) fn note_jobs(&mut self, events: &[Event]) {
        for event in events {
            let Body::ToolResult(result) = &event.body else {
                continue;
            };
            if self.nested.ports.remove(&result.call_id) {
                self.nested.closed.insert(result.call_id);
            }
            for effect in &result.effects {
                if let Effect::JobStarted(started) = effect
                    && started.what == JobKind::Command
                {
                    self.nested.started.insert(started.job.clone());
                }
            }
        }
        let held = std::mem::take(&mut self.nested.held);
        for back in held {
            // 先看它在等什么（借着看），再挪走它。
            let (waiting, given_up) = match &back {
                NestedBack::Call { id, origin, .. } => match id.parent() {
                    Parent::Job(job) => (
                        !self.nested.started.contains(job),
                        self.nested.closed.contains(origin),
                    ),
                    Parent::Call(_) => (false, false),
                },
                NestedBack::Done { .. } => (false, false),
            };
            match (waiting, given_up, back) {
                (true, true, NestedBack::Call { reply, .. }) => gone(reply),
                (true, _, back) => self.nested.held.push(back),
                (false, _, back) => send(&self.backs, back),
            }
        }
    }
}

/// 送回收件箱。会话停了就送不进去：等着的跟着丢，脚本那头读到没跑完。
#[expect(
    clippy::let_underscore_must_use,
    reason = "会话停了：等着的跟着丢，脚本那头读到没跑完"
)]
fn send(backs: &mpsc::UnboundedSender<Back>, back: NestedBack) {
    let _ = backs.send(Back::Nested(back));
}

/// 答「父级不在了」。脚本那头不等了的不管。
#[expect(clippy::let_underscore_must_use, reason = "脚本那头不等了：答了没人听")]
pub(super) fn gone(reply: oneshot::Sender<Nested>) {
    let _ = reply.send(Nested::Refused(miyu_kernel::session::NestedRefusal::Gone));
}
