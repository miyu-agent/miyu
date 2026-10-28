//! 扩展进程（施工 9-4 上，`docs/blueprint/extensions.md`）：核心拉起 `kind = "process"` 的包，经标准输入输出说同一套协议。
//!
//! 一个包一格：看管它的任务、任务报上来的状态。开关存在 `system/extensions.json`，没写的照清单的 `start`（`always` 的开着）。
//! 核心起来时照开关拉起开着的（[`Core::start_extensions`]），停的时候一起请它们退出（[`Core::stop_extensions`]）；有开着、没停下的，
//! 核心不算空闲。开、关、重启经协议（`methods.rs`），一件件办。看管一个包怎么拉起、退避、停下在 `supervise.rs`，标准错误的文件
//! 在 `stderr.rs`。

mod approval;
pub(crate) mod config;
mod instance;
pub(crate) mod methods;
mod offers;
mod ports;
mod state;
pub(crate) mod stderr;
mod supervise;

pub(crate) use approval::{described, forget as forget_switch, held};
pub(crate) use instance::Instance;
pub(crate) use methods::{disable, enable, entry, restart, status, write as write_switches};
pub(crate) use offers::offer;
pub(crate) use ports::{claim as claim_port, list as list_ports, release as release_port};
pub(crate) use state::{Cell, Reason, State, Status};
pub use supervise::Timing;
pub(crate) use supervise::backoff;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;

use miyu_config::package::{Manifest, PackageKind, Start};
use miyu_store::config_file;
use miyu_store::extensions::{self as file, Switches};
use miyu_store::packages::{Found, locate};

use crate::Core;

/// 运行日志的目标。
const TARGET: &str = "miyu::extensions";

type Shared = Arc<Cell>;

/// 广播最多攒多少条还没被读走的：读得慢的连接掉了队，推 `resync`。
const CHANGES: usize = 64;

/// 一份一格。
struct Slot {
    /// 状态。
    status: Shared,
    /// 看管它的任务：请它停的开关、它本身。没拉起过、请停了的没有。
    task: Option<(watch::Sender<bool>, JoinHandle<()>)>,
}

/// 扩展的表：核心一份。
pub(crate) struct Extensions {
    /// 每一份到它那一格（施工 IM-1 中起按（包，账号））。
    slots: Mutex<BTreeMap<Instance, Slot>>,
    /// 开、关、重启一件件办：两个连接同时开同一个，不拉起两个。
    ops: tokio::sync::Mutex<()>,
    /// 等多久、退避多久。
    timing: Timing,
    /// 谁变了（施工 9-4 补）。
    changed: broadcast::Sender<String>,
    /// 扩展登记的端口（施工 IM-5，`ports.rs`）。
    ports: ports::Ports,
    /// 扩展按账号交的预设功能（施工 IM-6，`offers.rs`）。
    pub(crate) offers: offers::Offers,
}

impl Extensions {
    /// 等多久、退避多久。
    pub(crate) fn timing(&self) -> Timing {
        self.timing
    }

    pub(crate) fn new(timing: Timing) -> Extensions {
        Extensions {
            slots: Mutex::new(BTreeMap::new()),
            ops: tokio::sync::Mutex::new(()),
            timing,
            changed: broadcast::channel(CHANGES).0,
            ports: ports::Ports::default(),
            offers: offers::Offers::default(),
        }
    }

    /// 开、关、重启一件件办：拿着它再读写开关（施工 X-3 上：`bundle` 的开关同一把锁）。
    pub(crate) async fn one_at_a_time(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.ops.lock().await
    }

    /// 收「谁变了」的一头（施工 9-4 补）。
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<String> {
        self.changed.subscribe()
    }

    /// 广播包 `id` 变了：开关变了、状态没变的也说一声（`enable` 一个程序没找到的，还是停下，`on` 变了）。
    pub(crate) fn notify(&self, id: &str) {
        if self.changed.send(id.to_string()).is_err() {
            // 没有订阅着的连接：不用说。
        }
    }

    fn slots(&self) -> std::sync::MutexGuard<'_, BTreeMap<Instance, Slot>> {
        self.slots.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 包 `id` 那一份（一份的）这时的状态：没拉起过的关着。
    pub(crate) fn status(&self, id: &str) -> Status {
        self.status_of(&Instance::of(id))
    }

    /// `instance` 这时的状态：没拉起过的关着。
    pub(crate) fn status_of(&self, instance: &Instance) -> Status {
        self.slots().get(instance).map_or(
            Status {
                state: State::Off,
                failures: 0,
            },
            |slot| slot.status.get(),
        )
    }

    /// 有在干活、或者等着再干的。
    pub(crate) fn busy(&self) -> bool {
        self.slots()
            .values()
            .any(|slot| slot.status.get().state.busy())
    }

    /// `instance` 那一格：没有的照关着建一格。广播的是包的编号。
    fn slot<'a>(
        &self,
        slots: &'a mut BTreeMap<Instance, Slot>,
        instance: &Instance,
    ) -> &'a mut Slot {
        slots.entry(instance.clone()).or_insert_with(|| Slot {
            status: Arc::new(Cell {
                id: instance.package.clone(),
                status: Mutex::new(Status {
                    state: State::Off,
                    failures: 0,
                }),
                changed: self.changed.clone(),
            }),
            task: None,
        })
    }

    /// 开着、不拉起的包 `id`（施工 9-4 下上：要的能力还有没批的）：一份的那一格记成停下，`reason` 照交的。
    fn hold(&self, id: &str, reason: Reason) {
        let mut slots = self.slots();
        self.slot(&mut slots, &Instance::of(id))
            .status
            .set(State::Stopped(reason), 0);
    }

    /// 拉起 `instance`（清单是 `manifest`）：在跑、在等的不动；程序没找到、协议版本对不上的直接停下。记成停下了的当没在跑：
    /// 看管的任务记完就交回，这时可能还没交回完（自己退了的那一份，等着它的马上要再拉起，施工 IM-1 下）。
    pub(crate) fn launch(&self, core: &Arc<Core>, instance: &Instance, manifest: &Manifest) {
        let mut slots = self.slots();
        let slot = self.slot(&mut slots, instance);
        if slot
            .task
            .as_ref()
            .is_some_and(|(_, task)| !task.is_finished())
            && !matches!(slot.status.get().state, State::Stopped(_))
        {
            return;
        }
        let plan = match plan(core, instance, manifest) {
            Ok(plan) => plan,
            Err(reason) => {
                tracing::warn!(target: TARGET, package = instance.label(), reason = reason.as_str(), "extension not started");
                slot.status.set(State::Stopped(reason), 0);
                slot.task = None;
                return;
            }
        };
        // 任务跑起来之前就算在干活：刚拉起的那一瞬间核心不算空闲。
        slot.status.set(State::Starting { pid: None }, 0);
        let (stop, asked) = watch::channel(false);
        let task = tokio::spawn(supervise::run(
            Arc::downgrade(core),
            plan,
            Arc::clone(&slot.status),
            asked,
            self.timing,
        ));
        slot.task = Some((stop, task));
    }

    /// 请包 `id` 的每一份退出、等它们退出（最多等到杀掉），状态记成关着：一份的，和每人一份的全部。
    async fn halt(&self, id: &str) {
        let tasks = self.take(|instance| instance.package == id);
        self.join(tasks).await;
    }

    /// 请全部退出、一起等。
    async fn halt_all(&self) {
        let tasks = self.take(|_| true);
        self.join(tasks).await;
    }

    /// 合 `which` 的那几格：请它们的任务停下，交回任务。
    fn take(&self, which: impl Fn(&Instance) -> bool) -> Vec<(Instance, Option<JoinHandle<()>>)> {
        self.slots()
            .iter_mut()
            .filter(|(instance, _)| which(instance))
            .map(|(instance, slot)| {
                let task = slot.task.take().map(|(stop, task)| {
                    if stop.send(true).is_err() {
                        // 任务自己停下了，没人收：不用管。
                    }
                    task
                });
                (instance.clone(), task)
            })
            .collect()
    }

    /// 等交回的任务退出，那几格记成关着、连续失败从零数。
    async fn join(&self, tasks: Vec<(Instance, Option<JoinHandle<()>>)>) {
        for (instance, task) in tasks {
            if let Some(task) = task
                && let Err(error) = task.await
            {
                tracing::error!(target: TARGET, package = instance.label(), panic = error.is_panic(), "extension task failed");
            }
            if let Some(slot) = self.slots().get(&instance) {
                slot.status.set(State::Off, 0);
            }
        }
    }
}

impl Core {
    /// 照开关拉起开着的扩展（施工 9-4 上）：核心起来、开始接连接之前调一次。要在 tokio 的运行时里调。要的能力还有没批的
    /// 不拉起，记成停下（`needs_approval`，施工 9-4 下上）。拉起以前照登记缓存先登记它的工具（施工 O-2 中）。
    pub fn start_extensions(self: &Arc<Self>) {
        // 扩展以系统账号的身份连进来：先建它们的家目录（施工 O-4 下）。
        crate::system_accounts::prepare(self);
        // 脚本登记进工具目录（施工 X-12 中）：拉起扩展以前，扩展登记的工具撞了脚本的名字的照扩展那边报。
        self.follow_scripts();
        // 个人设置里的 MCP 服务（施工 F-9 下）：起包里带的以前先定下谁盖住谁。
        self.note_home_mcp();
        let (switches, _) = read_switches(self);
        let packages = self.system_packages();
        // 依赖的包缺了的不起（施工 F-11）：`package.list` 的状态写明缺谁。
        let all: Vec<&Found> = packages.iter().collect();
        for (id, manifest) in processes_of(&all) {
            self.start_one(id, manifest, &switches);
        }
        // `bundle` 带的 MCP 服务同样照开关起（施工 X-3 上）；个人设置里的写了就起（施工 F-9 下）。
        self.start_mcp();
        self.start_home_mcp();
    }

    /// 开着的拉起：程序不在的不拉起，记成停下（`not_installed`，施工 F-6 上）；要的能力还有没批的不拉起，记成停下
    /// （`needs_approval`）；拉起以前照登记缓存先登记它的工具。
    fn start_one(self: &Arc<Self>, id: &str, manifest: &Manifest, switches: &Switches) {
        if !on(switches, id, manifest) {
            return;
        }
        // 程序不在的当没装（施工 F-6 上）：不照缓存登记它的工具，记成停下。
        if crate::packages::program_missing(self, id, manifest) {
            self.extensions.hold(id, Reason::NotInstalled);
            return;
        }
        match approval::unapproved(self, id, manifest).is_empty() {
            true => {
                crate::provide::restore(self, id);
                self.launch(id, manifest);
                // 它带的 MCP 服务跟着起（施工 X-3 下）。
                self.start_services(id, manifest);
            }
            false => self.extensions.hold(id, Reason::NeedsApproval),
        }
    }

    /// 拉起包 `id` 那一份。
    pub(crate) fn launch(self: &Arc<Self>, id: &str, manifest: &Manifest) {
        self.extensions.launch(self, &Instance::of(id), manifest);
    }

    /// 装卸时照清单从 `before` 换到 `now` 跟着拉起、停下扩展（施工 F-5 下，设计 30 第九节）。卸包是删文件以前先换到去掉它
    /// 以后的那一份，删不成的再换回来（施工 F-5 补）。
    /// - 卸掉的停下，它登记的工具拿掉、记下随包卸掉了：用过它的会话照旧留着、调到时报已卸载。
    /// - 新装上的照开关拉起；升级了的（清单变了）先停下，再照开关拉起。
    /// - 系统账号照核心这时的清单先建好家目录。
    pub(crate) async fn follow_packages(
        self: &Arc<Self>,
        old: &Processes<'_>,
        new: &Processes<'_>,
    ) {
        crate::system_accounts::prepare(self);
        for id in old.keys().filter(|id| !new.contains_key(*id)) {
            self.extensions.halt(id).await;
            // 它带的 MCP 服务也停下（施工 X-3 下）；工具和它自己的一起记成随包卸掉。
            self.mcp.halt(self, id).await;
            let removed = self
                .tools
                .replace(|catalog| Ok::<_, miyu_tool::CatalogError>(catalog.removing(id)));
            if let Err(error) = removed {
                tracing::error!(target: "miyu::endpoint", package = *id, error = %error, "tools not withdrawn");
            }
        }
        let (switches, _) = read_switches(self);
        for (id, manifest) in new {
            match old.get(id) {
                Some(previous) if previous == manifest => continue,
                Some(previous) => {
                    self.extensions.halt(id).await;
                    self.stop_services(id, previous).await;
                }
                None => {}
            }
            self.start_one(id, manifest, &switches);
        }
    }

    /// 请全部扩展、MCP 服务（施工 X-3 上）退出，等它们退出（最多等到杀掉）：核心停的时候调。
    pub async fn stop_extensions(&self) {
        self.extensions.halt_all().await;
        self.mcp.halt_all(self).await;
    }
}

/// 起来时读到的 `process` 包：编号和清单，照编号排。
/// 一份清单里的扩展包：编号到清单（施工 F-5 下、补：装卸时照它对）。
pub(crate) type Processes<'a> = BTreeMap<&'a str, &'a Manifest>;

/// 清单 `packages` 里起得来的扩展包，收成 [`Processes`]：依赖的包缺了的不算（施工 F-11），装卸时照它停下、拉起。
pub(crate) fn processes_of<'a>(packages: &[&'a Found]) -> Processes<'a> {
    packages
        .iter()
        .filter_map(|found| process(found))
        .filter(|(_, manifest)| crate::packages::depends::missing(packages, manifest).is_empty())
        .collect()
}

pub(crate) fn processes(packages: &[Found]) -> impl Iterator<Item = (&str, &Manifest)> {
    packages.iter().filter_map(process)
}

/// 读成了的扩展包：编号和清单。
fn process(found: &Found) -> Option<(&str, &Manifest)> {
    let manifest = found.read.as_ref().ok()?;
    (manifest.kind == PackageKind::Process).then_some((found.id.as_str(), manifest))
}

/// 包 `id` 的开关这时开没开（施工 F-6 上：`package.list` 的 `enabled`、`status` 照它）：照开关的文件。
pub(crate) fn switched_on(core: &Core, id: &str, manifest: &Manifest) -> bool {
    let (switches, _) = read_switches(core);
    on(&switches, id, manifest)
}

/// 包 `id` 开没开：写了的照写的，没写的照清单的 `start`。
fn on(switches: &Switches, id: &str, manifest: &Manifest) -> bool {
    switches.on.get(id).copied().unwrap_or_else(|| {
        manifest
            .process
            .as_ref()
            .is_some_and(|process| process.start == Start::Always)
    })
}

/// 开关的文件在哪。
fn switches_path(core: &Core) -> PathBuf {
    core.root.system().join(file::FILE)
}

/// 读开关：读不成的当没有，记一行 `WARN`；交回它的版本（写回时用），读不成的也交回整份字节的版本，好盖掉它。
pub(crate) fn read_switches(core: &Core) -> (Switches, Option<String>) {
    let path = switches_path(core);
    match file::read(&path) {
        Ok(read) => (read.switches, read.version),
        Err(error) => {
            tracing::warn!(target: TARGET, file = %path.display(), error = %error, "extension switches unreadable");
            let version = config_file::read(&path)
                .ok()
                .flatten()
                .map(|text| text.version);
            (Switches::default(), version)
        }
    }
}

/// 怎么拉起包 `id`：程序没找到、协议版本对不上的交回为什么。
fn plan(core: &Core, instance: &Instance, manifest: &Manifest) -> Result<supervise::Plan, Reason> {
    let id = instance.package.as_str();
    if crate::packages::mismatch(manifest).is_some() {
        return Err(Reason::ProtocolMismatch);
    }
    let main = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("miyu"));
    let dir = crate::packages::dir_of(core, id);
    let launch = manifest
        .command()
        .and_then(|command| locate(&command.program, &dir, &main))
        .ok_or(Reason::NotInstalled)?;
    let args = manifest
        .process
        .as_ref()
        .map(|process| process.args.clone())
        .unwrap_or_default();
    Ok(supervise::Plan {
        id: instance.label(),
        instance: instance.clone(),
        launch,
        args,
        dir: instance.state_dir(core),
        home: core.root.path().to_path_buf(),
        resources: core.resources.path().to_path_buf(),
        package: dir,
        log: stderr::path(core, &instance.label()),
    })
}
