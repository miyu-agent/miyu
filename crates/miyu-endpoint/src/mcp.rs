//! MCP 服务（施工 X-3 上、下，`docs/blueprint/mcp.md`「怎么走」第二到第四条）：核心起 `bundle` 包、扩展包带的服务，经标准
//! 输入输出说 MCP，列出来的工具登记进目录，她调到时转给那个服务。
//!
//! 一个服务一格：看管它的任务、任务报上来的状态、这时连着的客户端、目录什么时候过期。`bundle` 的开关同扩展
//! （`system/extensions.json`，没写的关着）；扩展包带的跟着扩展起停（施工 X-3 下）。核心起来时起开着的包的服务
//! （[`Core::start_extensions`] 里），停的时候一起关掉；有在跑、在等着重启的，核心不算空闲。怎么起一个在 `launch.rs`，看管在
//! `supervise.rs`，目录缓存在 `cache.rs`，工具名、规格在 `spec.rs`，一件工具在 `tool.rs`，结果换块在 `content.rs`，开关在
//! `switch.rs`。

mod cache;
mod content;
mod launch;
mod spec;
mod supervise;
mod switch;
mod tool;

pub(crate) use cache::forget;
pub(crate) use launch::stderr_file;
pub(crate) use switch::{turn_off, turn_on};

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::sync::{Notify, watch};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use miyu_config::package::{Manifest, PackageKind};
use miyu_mcp::Client;
use miyu_store::packages::Found;

use crate::Core;
use crate::extensions::{State, Status, Timing};

/// 运行日志的目标。
const TARGET: &str = "miyu::mcp";

/// 一个服务看管的任务和工具共用的：状态、这时连着的客户端。
pub(crate) struct Shared {
    /// 包的编号。
    package: String,
    /// 服务名。
    service: String,
    /// 状态。
    status: Mutex<Status>,
    /// 连着的客户端：起着、退避着、停下了的没有，调到的说暂时不可用。
    client: Mutex<Option<Arc<Client>>>,
    /// 目录什么时候过期（新时代的 `ttlMs`，施工 X-3 下）：过了以后下一次调用时叫看管的任务再列一次。没说的不过期。
    stale_at: Mutex<Option<Instant>>,
    /// 叫看管的任务再列一次。
    relist: Notify,
}

impl Shared {
    fn new(package: &str, service: &str) -> Shared {
        Shared {
            package: package.to_string(),
            service: service.to_string(),
            status: Mutex::new(Status {
                state: State::Off,
                failures: 0,
            }),
            client: Mutex::new(None),
            stale_at: Mutex::new(None),
            relist: Notify::new(),
        }
    }

    /// 列完了：目录 `fresh_for` 以后过期，没说的不过期。
    fn listed(&self, fresh_for: Option<Duration>) {
        *self.stale_at.lock().unwrap_or_else(PoisonError::into_inner) =
            fresh_for.map(|fresh_for| Instant::now() + fresh_for);
    }

    /// 要用它了（一次调用）：目录过期的叫看管的任务再列一次，这一次照旧调。
    pub(crate) fn touch(&self) {
        let stale = self
            .stale_at
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some_and(|at| Instant::now() >= at);
        if stale {
            self.listed(None);
            self.relist.notify_one();
        }
    }

    /// 这时的状态。
    fn get(&self) -> Status {
        *self.status.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 记下状态，告诉订阅着的连接这个包变了。
    fn set(&self, core: &Core, state: State, failures: u32) {
        *self.status.lock().unwrap_or_else(PoisonError::into_inner) = Status { state, failures };
        core.extensions.notify(&self.package);
    }

    /// 这时连着的客户端。
    pub(crate) fn client(&self) -> Option<Arc<Client>> {
        self.client
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 换上连着的客户端（断了、停了的换成没有）。
    fn connect(&self, client: Option<Arc<Client>>) {
        *self.client.lock().unwrap_or_else(PoisonError::into_inner) = client;
    }
}

/// 一个服务一格。
struct Slot {
    shared: Arc<Shared>,
    /// 看管它的任务：请它停的开关、它本身。没起过、停了的没有。
    task: Option<(watch::Sender<bool>, JoinHandle<()>)>,
}

/// 服务的表：核心一份，照包的编号、服务名排。
pub(crate) struct Servers {
    slots: Mutex<BTreeMap<(String, String), Slot>>,
    timing: Timing,
}

impl Servers {
    pub(crate) fn new(timing: Timing) -> Servers {
        Servers {
            slots: Mutex::new(BTreeMap::new()),
            timing,
        }
    }

    fn slots(&self) -> std::sync::MutexGuard<'_, BTreeMap<(String, String), Slot>> {
        self.slots.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 有在跑、在起、在等着重启的。
    pub(crate) fn busy(&self) -> bool {
        self.slots()
            .values()
            .any(|slot| slot.shared.get().state.busy())
    }

    /// 包 `package` 的服务 `service` 这时的状态（施工 X-4，软件包列表上一个服务一格）：没起过的关着。
    pub(crate) fn service(&self, package: &str, service: &str) -> Status {
        let key = (package.to_string(), service.to_string());
        self.slots().get(&key).map_or(
            Status {
                state: State::Off,
                failures: 0,
            },
            |slot| slot.shared.get(),
        )
    }

    /// 包 `package` 的服务合起来在哪一步：有停下的算停下；都在跑的算在跑；别的（起着、退避着、还没起过）算起着。没有服务
    /// 的没有。
    pub(crate) fn state(&self, package: &str, manifest: &Manifest) -> Option<State> {
        let slots = self.slots();
        let states: Vec<State> = manifest
            .mcp
            .iter()
            .map(|server| {
                slots
                    .get(&(package.to_string(), server.name.clone()))
                    .map_or(State::Off, |slot| slot.shared.get().state)
            })
            .collect();
        if states.is_empty() {
            return None;
        }
        if let Some(stopped) = states
            .iter()
            .find(|state| matches!(state, State::Stopped(_)))
        {
            return Some(*stopped);
        }
        if let Some(running) = states
            .iter()
            .find(|state| matches!(state, State::Running { .. }))
            .filter(|_| {
                states
                    .iter()
                    .all(|state| matches!(state, State::Running { .. }))
            })
        {
            return Some(*running);
        }
        Some(State::Starting { pid: None })
    }

    /// 起包 `package` 的服务（清单是 `manifest`）：在跑、在等的不动。
    fn launch(&self, core: &Arc<Core>, package: &str, manifest: &Manifest) {
        let dir = crate::packages::dir_of(core, package);
        let mut slots = self.slots();
        for server in &manifest.mcp {
            let key = (package.to_string(), server.name.clone());
            let slot = slots.entry(key).or_insert_with(|| Slot {
                shared: Arc::new(Shared::new(package, &server.name)),
                task: None,
            });
            if slot
                .task
                .as_ref()
                .is_some_and(|(_, task)| !task.is_finished())
            {
                continue;
            }
            let mut plan = launch::plan(core, package, server, &dir);
            // 照缓存先登记（施工 X-3 下）：服务好以前调到的说暂时不可用；连的时候照记着的时代直接说。
            if let Some((era, tools)) = cache::read(core, server) {
                spec::register(core, package, server, &tools, &slot.shared);
                plan.known = Some(era);
            }
            // 任务跑起来之前就算在干活：刚起的那一瞬间核心不算空闲。
            slot.shared.set(core, State::Starting { pid: None }, 0);
            let (stop, asked) = watch::channel(false);
            let task = tokio::spawn(supervise::run(
                Arc::downgrade(core),
                plan,
                Arc::clone(&slot.shared),
                asked,
                self.timing,
            ));
            slot.task = Some((stop, task));
        }
    }

    /// 关掉包 `package` 的服务：请它们退出、等它们退出（最多等到杀掉），记成关着。工具留在目录里：要它们出目录的用
    /// [`Core::stop_services`]，卸包的照随包卸掉记。
    pub(crate) async fn halt(&self, core: &Core, package: &str) {
        let tasks: Vec<(Arc<Shared>, Option<JoinHandle<()>>)> = self
            .slots()
            .iter_mut()
            .filter(|((owner, _), _)| owner == package)
            .map(|(_, slot)| (Arc::clone(&slot.shared), ask_to_stop(slot)))
            .collect();
        for (shared, task) in tasks {
            settle(core, &shared, task).await;
        }
    }

    /// 关掉全部、一起等：核心停的时候调。
    pub(crate) async fn halt_all(&self, core: &Core) {
        let tasks: Vec<(Arc<Shared>, Option<JoinHandle<()>>)> = self
            .slots()
            .values_mut()
            .map(|slot| (Arc::clone(&slot.shared), ask_to_stop(slot)))
            .collect();
        for (shared, task) in tasks {
            settle(core, &shared, task).await;
        }
    }
}

/// 请一格的任务停：交回它，等它的一方去等。
fn ask_to_stop(slot: &mut Slot) -> Option<JoinHandle<()>> {
    let (stop, task) = slot.task.take()?;
    if stop.send(true).is_err() {
        // 任务自己停下了，没人收：不用管。
    }
    Some(task)
}

/// 等一格的任务停下，记成关着、断开客户端。
async fn settle(core: &Core, shared: &Shared, task: Option<JoinHandle<()>>) {
    if let Some(task) = task
        && let Err(error) = task.await
    {
        tracing::error!(target: TARGET, package = shared.package.as_str(), service = shared.service.as_str(), error = %error, "mcp task failed");
    }
    shared.connect(None);
    shared.set(core, State::Off, 0);
}

impl Core {
    /// 起包 `id` 带的服务（施工 X-3 下：`bundle` 开了、扩展拉起了都走这里）：在跑、在等的不动。
    pub(crate) fn start_services(self: &Arc<Self>, id: &str, manifest: &Manifest) {
        if !manifest.mcp.is_empty() {
            self.mcp.launch(self, id, manifest);
        }
    }

    /// 重启包 `id` 带的服务（施工 X-4，`package.restart`）：请它们退出、等它们退出，再起，连续失败从零数；工具留在目录里，
    /// 起好、列完照新的换。
    pub(crate) async fn restart_services(self: &Arc<Self>, id: &str, manifest: &Manifest) {
        if !manifest.mcp.is_empty() {
            self.mcp.halt(self, id).await;
            self.start_services(id, manifest);
        }
    }

    /// 停下包 `id` 带的服务，它们的工具出目录（清单是 `manifest`）：开着的会话下一个回合换掉。
    pub(crate) async fn stop_services(&self, id: &str, manifest: &Manifest) {
        if !manifest.mcp.is_empty() {
            self.mcp.halt(self, id).await;
            switch::withdraw(self, id, manifest);
        }
    }

    /// 起开着的 `bundle` 包带的服务（施工 X-3 上）：核心起来时在 [`Core::start_extensions`] 里调。扩展包带的随扩展拉起。
    pub(crate) fn start_mcp(self: &Arc<Self>) {
        let packages = self.packages();
        for found in packages.iter() {
            if let Some(manifest) = bundle(found)
                && crate::extensions::switched_on(self, &found.id, manifest)
            {
                self.start_services(&found.id, manifest);
            }
        }
    }

    /// 装卸时照清单从 `before` 换到 `now` 跟着停、起 `bundle` 带的服务（施工 X-3 上，同扩展的 `follow_packages`）：卸掉的
    /// 停下，工具记成随包卸掉；升级了的（清单变了）先停下、工具出目录；新装上的、升级了的照开关起。
    pub(crate) async fn follow_bundles(self: &Arc<Self>, before: &[&Found], now: &[&Found]) {
        let old = bundles_of(before);
        let new = bundles_of(now);
        for id in old.keys().filter(|id| !new.contains_key(*id)) {
            self.mcp.halt(self, id).await;
            let removed = self
                .tools
                .replace(|catalog| Ok::<_, miyu_tool::CatalogError>(catalog.removing(id)));
            if let Err(error) = removed {
                tracing::error!(target: TARGET, package = *id, error = %error, "tools not withdrawn");
            }
        }
        for (id, manifest) in &new {
            match old.get(id) {
                Some(previous) if previous == manifest => continue,
                Some(previous) => self.stop_services(id, previous).await,
                None => {}
            }
            if crate::extensions::switched_on(self, id, manifest) {
                self.start_services(id, manifest);
            }
        }
    }
}

/// 读成了的 `bundle` 包的清单。
fn bundle(found: &Found) -> Option<&Manifest> {
    found
        .read
        .as_ref()
        .ok()
        .filter(|manifest| manifest.kind == PackageKind::Bundle)
}

/// 一份清单里的 `bundle` 包：编号到清单。
fn bundles_of<'a>(packages: &[&'a Found]) -> BTreeMap<&'a str, &'a Manifest> {
    packages
        .iter()
        .filter_map(|found| Some((found.id.as_str(), bundle(found)?)))
        .collect()
}
