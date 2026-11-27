//! 会话表（`internal/designs/07-存储.md` 第七节「会话按需载入」）：照编号找会话；这次运行里没在跑的，从磁盘
//! 载入；停了的、闲够了退下的拿掉，下次用到再载入（退下在 `sessions/idle.rs`，施工 V-2 再补）。删会话连子会话在
//! `sessions/delete.rs`（施工 3-8 三补）。造会话在 `sessions/creating.rs`（施工 CS-9 挪出去）。
//!
//! 表拿 tokio 的锁护着，载入期间一直拿着：两个连接同时说给同一个没在跑的会话，只载入一次、只起一个
//! actor（一个会话只能有一个写者，`07-存储.md` 第三节）。造会话也拿着，可核心要停时不拿着它等写盘（`sessions/stopping.rs`）。

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use tokio::sync::Mutex;

use miyu_kernel::facts::Environment;
use miyu_kernel::id::{CommandId, SessionId, VenueId};
use miyu_kernel::origin::{By, Person, Session};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_policy::Profile;
use miyu_policy::memory::MemoryScope;
use miyu_session::{Child, Create, ForkedBase, Handle, create, new_id};

use crate::Core;
use crate::personas;
use crate::presets;
use crate::refusal::Refusal;
use crate::spawn;

mod advisor;
mod asked;
mod confined;
mod creating;
mod delete;
mod dream;
mod forked;
mod found;
mod held;
mod idle;
mod opening;
mod orphans;
mod peek;
mod places;
pub(crate) mod prefix;
mod pressure;
mod setup;
pub(crate) mod state;
mod stopping;

pub(crate) use confined::{confined_of, placed, reachable, reaches};
pub(crate) use opening::Opening;
pub(crate) use peek::peeks;
pub(crate) use places::{Picked, check_dirs, opening_cwd, pick, roots, to_default, wide};
pub(crate) use setup::Setup;
#[cfg(test)]
mod tests;

/// 放在堆上的一步（施工 LF-12 修；那一步里最深的一条路施工 RM-1 拆了，新开会话、命令分发这几步照旧装箱）：新开会话
/// 的状态很大，写成普通的 `async fn` 的话，调的一方那一帧里要先放下整份状态；调试版里每一处 `.await` 各占一份，从连接一路
/// 叠下来新开会话本身就要 800 KiB 上下的栈。交回装好箱的，调的一方只拿一个指针。
pub(crate) type Heaped<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

/// 会话表。
#[derive(Debug)]
pub(crate) struct Sessions {
    open: Mutex<Open>,
    /// 闲多久退下（施工 V-2 再补）：出厂 [`idle::IDLE`]，测试里设短的。
    idle: std::time::Duration,
    /// 看空闲的任务起了没有：第一次往表里放会话时起。
    sweeping: std::sync::atomic::AtomicBool,
    /// 封存了、报了退得下了的会话（施工 V-51，`sessions/idle.rs`）：看空闲的任务醒来当场问它们。
    due: Arc<idle::Due>,
    /// 正在答的顾问会话（施工 ADV-1，`sessions/advisor.rs`）：同一个顾问会话一次只答一个问题。
    answering: std::sync::Mutex<BTreeSet<SessionId>>,
    /// 内存紧时把闲着的会话卸出内存（施工 MEM-2，`sessions/pressure.rs`）：怎么读、隔多久看一眼、开了会话叫一声。
    pressure: pressure::Pressure,
    /// 核心要停了（施工 CS-9，`sessions/stopping.rs`）：正在造的会话不再拿着表的锁等写盘，停的时候还在写的交给它等完、停下。
    stopping: stopping::Stopping,
}

#[derive(Debug, Default)]
struct Open {
    /// 在跑的会话，和它现在的工作目录。
    running: BTreeMap<SessionId, Running>,
    /// 最近造会话的命令编号，和它造出的会话、造它的参数的指纹（[`Opening::digest`]，施工 YC-7；重启以后补回来的没有），
    /// 照从旧到新。
    created: VecDeque<(CommandId, SessionId, Option<u64>)>,
    /// 这次运行里补回过去重的编号没有：第一次造会话时补一次。
    recalled: bool,
}

#[derive(Debug)]
struct Running {
    handle: Handle,
    /// 实际在哪个目录里干活：头报的太宽的，是 `workspace/`。施工 9-7 上起只有人换工作区才变（[`Sessions::moved`]）。
    workspace: String,
    /// 加进来的目录（施工 5-10 上）。同上。
    dirs: Vec<String>,
}

impl Sessions {
    /// 有没有会话 `id`：在跑的、磁盘上有目录的都算。不拿表的锁：删会话拿着锁时，父会话经端口读子会话的日志也要问这一句。
    pub(crate) async fn exists(&self, core: &Core, id: &SessionId) -> bool {
        stored(core, id).await
    }
}

impl Open {
    /// 有没有会话 `id`，表的锁在调的一方手里：在跑的算，没在跑的照磁盘（[`stored`]）。
    pub(super) async fn exists(&self, core: &Core, id: &SessionId) -> bool {
        self.running.contains_key(id) || stored(core, id).await
    }
}

/// 磁盘上有没有会话 `id`：在阻塞线程里看目录在不在。
pub(super) async fn stored(core: &Core, id: &SessionId) -> bool {
    let (root, id) = (core.root.clone(), id.clone());
    let there = tokio::task::spawn_blocking(move || root.session_dir(&id).is_dir()).await;
    matches!(there, Ok(true))
}

/// 造好的会话：编号，和它实际在哪个目录里干活（施工 4-5 下）；这个目录的项目配置还没问过信不信任的，它在哪（施工 8-2）。
#[derive(Debug)]
pub(crate) struct Created {
    pub(crate) id: SessionId,
    pub(crate) cwd: String,
    /// 太宽、照人选的用着（施工 9-7 补）。
    pub(crate) wide: bool,
    pub(crate) untrusted: Option<String>,
}

/// 找到的会话：把手，和它这会儿实际在哪个目录里干活（施工 4-5 下）。
#[derive(Debug)]
pub(crate) struct Found {
    pub(crate) handle: Handle,
    pub(crate) cwd: String,
}

impl Sessions {
    /// 会话 `id` 在跑的话交回它的把手，没在跑的不载入、交回空的（施工 9-8 补下：算会话树时只看载入了的）。
    pub(crate) async fn loaded(&self, id: &SessionId) -> Option<Handle> {
        let open = self.open.lock().await;
        open.running.get(id).map(|running| running.handle.clone())
    }

    /// 找会话 `id`：在跑的直接交回；没在跑的从磁盘载入。施工 9-7 上起头每句话报的工作目录不再换会话的：换工作区另走
    /// `session.set_workspace`（`workspace.rs`）。
    pub(crate) async fn get(&self, core: &Arc<Core>, id: &SessionId) -> Result<Found, Refusal> {
        let mut open = self.open.lock().await;
        let found = open.found(core, id, None).await;
        self.sweep(core);
        found
    }

    /// 同 [`Sessions::get`]，会话 `id` 是刚分叉出来的支线，`base` 是分叉那一遍读好的底子（施工 V-39 下）：要从磁盘载入的
    /// 交给载入，不再读父会话；已经在跑的（别的路先载入了）用不上，跟着放掉。
    pub(crate) async fn get_forked(
        &self,
        core: &Arc<Core>,
        id: &SessionId,
        base: ForkedBase,
    ) -> Result<Found, Refusal> {
        let mut open = self.open.lock().await;
        let found = open.found(core, id, Some(base)).await;
        self.sweep(core);
        found
    }

    /// 会话 `id` 换了工作区（施工 9-7 上）：会话表记着的跟着换（说话的回应照它写 `cwd`）。写了 `dirs` 的整份换掉。交回现在
    /// 加进来的目录；不在表里的（停了）没有。
    pub(crate) async fn moved(
        &self,
        id: &SessionId,
        workspace: String,
        dirs: Option<Vec<String>>,
    ) -> Option<Vec<String>> {
        let mut open = self.open.lock().await;
        let running = open.running.get_mut(id)?;
        running.workspace = workspace;
        if let Some(dirs) = dirs {
            running.dirs = dirs;
        }
        Some(running.dirs.clone())
    }

    /// 造一个子会话（施工 7-5，`agents.md` 第一条）：照执行器填好的 `child`，由父会话造（`by` 是它）。放进表里，和头造的
    /// 一样照编号找得到：一个会话只起一个 actor。造不成的交回原因，由执行器记进运行日志。
    pub(crate) async fn spawn(&self, core: &Arc<Core>, child: Child) -> Result<SessionId, String> {
        let mut open = self.open.lock().await;
        // 父会话被删了（施工 3-8 三补）：它停下之前在派的不再造，不留下没有父会话的子会话。
        if !open.running.contains_key(&child.lineage.parent) {
            return Err("the parent session is gone".to_string());
        }
        let id = new_id(now());
        let parent = By::Session(Session {
            id: child.lineage.parent.clone(),
        });
        // 没挑人格的子代理无人格（施工 P-4 上），不照默认人格。
        let persona = personas::resolve(core, Some(child.persona.as_deref()))
            .await
            .map_err(|refusal| format!("persona {:?}: {}", child.persona, refusal.reason))?;
        // 子会话照父会话的预设（施工 P-2 中）：照它的编号重新找；找不到、写错了的不造，同人格。
        let preset = match &child.preset {
            Some(id) => Some(presets::chosen(
                core,
                presets::resolve(core, Some(id))
                    .await
                    .map_err(|refusal| format!("preset {id}: {}", refusal.reason))?,
            )),
            None => None,
        };
        let presets_of = presets::places(core);
        let resources = presets::overrides::resources(core);
        let handle = create(Create {
            root: &core.root,
            resources: &resources,
            id: id.clone(),
            persona: persona.as_ref().map(|found| found.id.as_str()),
            persona_texts: persona
                .as_ref()
                .map(|found| found.texts.clone())
                .unwrap_or_default(),
            profile: Profile::default(),
            personas: personas::personas(core),
            memory_scope: MemoryScope::Off,
            venue: child.venue,
            index: Some(core.index()),
            usage: core.ledger(),
            knowledge: Some(Arc::clone(&core.knowledge)),
            vectors: core.memory.vectors().cloned(),
            installed: Some(crate::packages::installed(core)),
            plugins: core.plugins.clone(),
            memory: Some(Arc::clone(&core.memory)),
            configs: core.configs_of(),
            permission: child.permission,
            attended: child.attended,
            oneshot: false,
            environment: environment(child.cwd.clone(), child.dirs.clone()),
            command: child.command,
            by: parent,
            models: &*core.models,
            tools: core.tools.clone(),
            home: core.home.as_deref(),
            sandbox: core.sandbox.helper(),
            lineage: Some(child.lineage),
            sessions: Some(spawn::port(core)),
            packages: Some(crate::packages::port::port(core, child.confined)),
            schedules: Some(crate::schedules::port::port(core)),
            hooks: Some(crate::hooks::port(core)),
            jobs: &core.jobs,
            model: child.model,
            preset,
            no_tools: false,
            presets: Some(presets_of),
            group: None,
            confined: child.confined,
            images: child.images,
            titles: true,
            dream: None,
            origin: None,
            voice_max_ms: None,
        })
        .await
        .map_err(|error| error.to_string())?;
        // 工作目录是父会话这一轮实际干活的那一个，已经定过宽不宽。
        let running = Running {
            handle,
            workspace: child.cwd,
            dirs: child.dirs,
        };
        open.running.insert(id.clone(), running);
        self.opened();
        Ok(id)
    }

    /// 会话 `id` 停了：从表里拿掉，下次用到再载入。
    pub(crate) async fn forget(&self, id: &SessionId) {
        self.open.lock().await.running.remove(id);
    }

    /// 有没有在跑的回合：哪个在跑的会话还忙着，就是有（施工 3-9 上）。
    pub(crate) async fn busy(&self) -> bool {
        let open = self.open.lock().await;
        open.running.values().any(|running| running.handle.busy())
    }

    /// 这时忙着的会话（施工 C-3）：在表里、有回合在进行，和 [`Sessions::busy`] 看的是同一样。列会话时照它写忙不忙。
    pub(crate) async fn busy_ids(&self) -> BTreeSet<SessionId> {
        let open = self.open.lock().await;
        open.running
            .iter()
            .filter(|(_, running)| running.handle.busy())
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// 这时在等人确认、回答的会话（施工 SL-1）：在表里、`Handle::waiting` 的。列会话时照它写 `waiting`。
    pub(crate) async fn waiting_ids(&self) -> BTreeSet<SessionId> {
        let open = self.open.lock().await;
        open.running
            .iter()
            .filter(|(_, running)| running.handle.waiting())
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// 有计划地停下全部在跑的会话：跑到一半的回合记成「重启了」，下次载入接着干（施工 3-9 上）。先说要停了（施工 CS-9）：
    /// 正在造的会话不再等写盘、放开表的锁（`sessions/creating.rs`），这里拿得到锁；停的时候还在写的，和在跑的一起停。
    pub(crate) async fn stop_all(&self) {
        self.stopping.begin();
        let running = std::mem::take(&mut self.open.lock().await.running);
        let stop_running = async {
            for (id, running) in running {
                if running.handle.stop().await.is_err() {
                    tracing::debug!(target: "miyu::endpoint", session = id.as_str(), "already stopped");
                }
            }
        };
        tokio::join!(stop_running, self.stopping.settle());
    }
}

/// 主人：连上来的、核心替谁做事都记成他（施工 SU-4 下）。
pub(crate) fn person() -> By {
    By::Person(Person::default())
}

/// 主人在种类是 `head` 的头上说的话（施工 VN-1，[`Person::head`]）。
pub(crate) fn person_on(head: Option<miyu_kernel::id::HeadKind>) -> By {
    By::Person(Person { via: None, head })
}

/// 本机这个场所。
fn local() -> VenueId {
    VenueId::parse("local").unwrap_or_else(|e| unreachable!("「local」合场所的写法：{e}"))
}

/// 会话所在的环境：核心所在的机器现在的时区，实际干活的目录（[`opening_cwd`] 定的），加进来的目录（施工 5-10 上）。
fn environment(workspace: String, dirs: Vec<String>) -> Environment {
    Environment {
        offset: offset(),
        cwd: workspace,
        dirs,
    }
}

/// 核心所在的机器现在的时区偏移，到分钟。读不出来的当 UTC。
pub(crate) fn offset() -> UtcOffset {
    let minutes = miyu_log::zone::now().offset().seconds() / 60;
    UtcOffset::from_minutes(minutes)
        .or_else(|| UtcOffset::from_minutes(0))
        .unwrap_or_else(|| unreachable!("UTC 在偏移的范围里"))
}

/// 现在：造会话编号用，编号的前 48 位是它；配置的日志也照它记时刻（施工 8-3）。
pub(crate) fn now() -> Timestamp {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| i64::try_from(since.as_millis()).unwrap_or(0));
    Timestamp::from_unix_millis(millis)
        .or_else(|| Timestamp::from_unix_millis(0))
        .unwrap_or_else(|| unreachable!("1970 年在时刻的范围里"))
}
