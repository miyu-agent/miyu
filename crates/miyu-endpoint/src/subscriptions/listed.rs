//! 照编号推整项的几个列表的订阅：扩展的状态（施工 9-4 补，`docs/blueprint/extensions.md`「推送」）、软件包列表（施工 F-8 三补，
//! `protocol.md`「软件包列表的推送」）、这个账号的项目（施工 H-1，设计 `38-项目.md` 第二节第 1 条）。一个连接各至多一个，一个转发任务。订阅的回应也交给它，它先写回应、再转推送：收到
//! 「哪个包变了」，照这一刻的样子、这个连接这一刻的语言算那一项写出去。推的是整项、照这一刻算，晚到的、重复的都对，不用编号排
//! 先后。读得慢、掉了队，推一条 `resync`（`{"stream": "extensions"}`、`{"stream": "packages"}`），这个订阅就停了，头重新订阅。
//! 再订阅总是换一个新的。

use std::sync::Arc;

use serde_json::json;
use tokio::sync::broadcast::{self, error::RecvError};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use super::Subscriptions;
use crate::Core;
use crate::hello::Shaken;

/// 哪一个列表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Listed {
    /// 扩展的状态：推 `extension.changed`。
    Extensions,
    /// 软件包列表：推 `package.changed`；扩展的状态变了，列表里那一项的状态、开关跟着变，也推。
    Packages,
    /// 项目：推 `projects.changed`。
    Projects,
    /// 定时任务（施工 J-1 下）：推 `schedules.changed`。
    Schedules,
    /// 知识库建索引的进度（施工 W-15，`knowledge/progress.rs`）：推 `kb.indexing`、`kb.indexed`。
    Knowledge,
    /// 通知（施工 J-3 下，`schedules/notices.rs`）：推 `notice`。
    Notices,
    /// 包推给人的（施工 IM-1 中下二，`backstage/events.rs`）：推 `package.event`，别的包的不推。
    PackageEvents(String),
}

impl Listed {
    /// 订阅的流叫什么。
    fn stream(&self) -> &'static str {
        match self {
            Listed::Extensions => "extensions",
            Listed::Packages => "packages",
            Listed::Projects => "projects",
            Listed::Schedules => "schedules",
            Listed::Knowledge => "kb",
            Listed::Notices => "notices",
            Listed::PackageEvents(..) => "package",
        }
    }

    /// 掉了队推的 `resync` 的参数：照包分的带上是哪个包。
    fn resync(&self) -> serde_json::Value {
        match self {
            Listed::PackageEvents(package) => json!({"stream": "package", "package": package}),
            _ => json!({"stream": self.stream()}),
        }
    }

    /// 收「哪个包变了」的几头。
    fn receivers(
        &self,
        core: &Core,
    ) -> (
        broadcast::Receiver<String>,
        Option<broadcast::Receiver<String>>,
    ) {
        match self {
            Listed::Extensions => (core.extensions.subscribe(), None),
            Listed::Packages => (
                core.package_changes.subscribe(),
                Some(core.extensions.subscribe()),
            ),
            Listed::Projects => (core.project_changes.subscribe(), None),
            Listed::Schedules => (core.schedules().subscribe(), None),
            Listed::Knowledge => (core.kb_progress.subscribe(), None),
            Listed::Notices => (core.schedules().notices().subscribe(), None),
            Listed::PackageEvents(..) => (core.backstage.events.subscribe(), None),
        }
    }

    /// 包 `id` 变了推的那一行。扩展的状态里没有这个包的不推；软件包列表里没有了的推 `entry: null`。
    fn line(&self, core: &Core, id: &str, shaken: &Shaken) -> Option<String> {
        let line = match self {
            // 进度那一行广播时就写好了（施工 W-15）。通知那一行同样广播时就写好了（施工 J-3 下）。
            Listed::Knowledge | Listed::Notices => return Some(id.to_string()),
            Listed::PackageEvents(package) => {
                return crate::backstage::mine(id, package).map(str::to_string);
            }
            Listed::Projects => {
                let params = crate::projects::item(core, id)?;
                json!({"jsonrpc": "2.0", "method": "projects.changed", "params": params})
            }
            Listed::Schedules => {
                let params = crate::schedules::methods::changed(core, id)?;
                json!({"jsonrpc": "2.0", "method": "schedules.changed", "params": params})
            }
            Listed::Extensions => {
                let entry = crate::extensions::entry(core, id, shaken.now(core))?;
                json!({"jsonrpc": "2.0", "method": "extension.changed", "params": {"entry": entry}})
            }
            // 个人设置里的 MCP 服务（施工 F-9 补）：推 `mcp.changed`，删掉了的 `server` 是 `null`。
            Listed::Packages => match crate::mcp::home::server_of(id) {
                Some(name) => {
                    let server = crate::mcp::methods::entry(core, name);
                    json!({"jsonrpc": "2.0", "method": "mcp.changed", "params": {"name": name, "server": server}})
                }
                None => {
                    let entry = crate::packages::entry(core, id, shaken.now(core));
                    json!({"jsonrpc": "2.0", "method": "package.changed", "params": {"entry": entry, "package": id}})
                }
            },
        };
        Some(line.to_string())
    }
}

/// 一个列表的订阅的转发任务。
#[derive(Debug)]
pub(super) struct ListedForwarder {
    /// 交订阅的回应给它的那一头：只交一次。
    reply: Option<oneshot::Sender<String>>,
    /// 转发任务：连接断了、换了新的、取消了跟着停。
    task: JoinHandle<()>,
}

impl ListedForwarder {
    /// 起一个：先拿收推送的几头（排在这之后的变化照样推），等订阅的回应、写出去，再转推送，照 `shaken` 这个连接这一刻的语言
    /// 算，写进 `out`。
    pub(super) fn start(
        core: Arc<Core>,
        listed: Listed,
        shaken: Shaken,
        out: mpsc::Sender<String>,
    ) -> ListedForwarder {
        let (reply, answer) = oneshot::channel();
        let pushes = listed.receivers(&core);
        let task = tokio::spawn(forward(core, listed, shaken, pushes, answer, out));
        ListedForwarder {
            reply: Some(reply),
            task,
        }
    }

    /// 交订阅的回应；已经交过、它已经不收了的，交回这一行。
    pub(super) fn reply(&mut self, line: String) -> Result<(), String> {
        match self.reply.take() {
            Some(reply) => reply.send(line),
            None => Err(line),
        }
    }
}

impl Drop for ListedForwarder {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// 转发：回应先写，再一条条推；掉了队推 `resync` 停下。
async fn forward(
    core: Arc<Core>,
    listed: Listed,
    shaken: Shaken,
    (mut first, mut second): (
        broadcast::Receiver<String>,
        Option<broadcast::Receiver<String>>,
    ),
    answer: oneshot::Receiver<String>,
    out: mpsc::Sender<String>,
) {
    let Ok(reply) = answer.await else {
        return;
    };
    if out.send(reply).await.is_err() {
        return;
    }
    loop {
        let got = tokio::select! {
            got = first.recv() => got,
            got = next(&mut second) => got,
        };
        match got {
            Ok(id) => {
                let Some(line) = listed.line(&core, &id, &shaken) else {
                    continue;
                };
                if out.send(line).await.is_err() {
                    break;
                }
            }
            Err(RecvError::Lagged(_)) => {
                tracing::warn!(target: "miyu::endpoint", stream = listed.stream(), "lagged, resync");
                let line = json!({"jsonrpc": "2.0", "method": "resync", "params": listed.resync()});
                if out.send(line.to_string()).await.is_err() {
                    // 连接断了：没人收。
                }
                break;
            }
            Err(RecvError::Closed) => break,
        }
    }
}

/// 第二头的下一条；没有第二头的永远等着。
async fn next(receiver: &mut Option<broadcast::Receiver<String>>) -> Result<String, RecvError> {
    match receiver {
        Some(receiver) => receiver.recv().await,
        None => std::future::pending().await,
    }
}

impl Subscriptions {
    /// 订阅一个照编号推整项的列表（扩展的状态，施工 9-4 补；软件包列表，施工 F-8 三补）：先起新的转发任务（它先拿收推送的
    /// 几头，排在这之前的变化照样推，推的是这一刻的整项，重复了也对）替掉原来的，再算一份整的；交回订阅的回应
    /// （`{"extensions": […]}`、`{"packages": […]}`），它经 [`super::Target::Extensions`]、[`super::Target::Packages`] 交给新的转发
    /// 任务写出去。`shaken` 是这个连接握手时记下的：推的时候照这一刻的语言挑名字。
    pub(crate) fn add_listed(
        &mut self,
        core: &Arc<Core>,
        packages: bool,
        shaken: Shaken,
        out: &mpsc::Sender<String>,
    ) -> Result<serde_json::Value, crate::refusal::Refusal> {
        let (listed, slot) = match packages {
            true => (Listed::Packages, &mut self.packages),
            false => (Listed::Extensions, &mut self.extensions),
        };
        *slot = Some(ListedForwarder::start(
            Arc::clone(core),
            listed.clone(),
            shaken.clone(),
            out.clone(),
        ));
        match listed {
            // 照订的人是不是管理员裁格子（施工 U23-R修二：原来的 `now` 恒当不是，管理员拿到的也是成员那一份）。
            Listed::Packages => crate::packages::list(core, shaken.now(core)),
            _ => Ok(crate::extensions::status(core, shaken.now(core))),
        }
    }

    /// 订阅项目（施工 H-1）：同 [`Subscriptions::add_listed`]，回应 `{"projects": […]}` 经
    /// [`super::Target::Projects`] 写出去。
    pub(crate) async fn add_projects(
        &mut self,
        core: &Arc<Core>,
        shaken: Shaken,
        out: &mpsc::Sender<String>,
    ) -> Result<serde_json::Value, crate::refusal::Refusal> {
        self.projects = Some(ListedForwarder::start(
            Arc::clone(core),
            Listed::Projects,
            shaken,
            out.clone(),
        ));
        crate::projects::listed(core).await
    }

    /// 取消订阅扩展的状态（`packages` 是假的）、软件包列表的推送：转发任务当场停。
    pub(crate) fn remove_listed(&mut self, packages: bool) {
        match packages {
            true => self.packages = None,
            false => self.extensions = None,
        }
    }

    /// 取消订阅项目的推送。
    pub(crate) fn remove_projects(&mut self) {
        self.projects = None;
    }

    /// 订阅定时任务（施工 J-1 下）：同 [`Subscriptions::add_projects`]，回应 `{"schedules": […]}`。
    pub(crate) fn add_schedules(
        &mut self,
        core: &Arc<Core>,
        shaken: Shaken,
        out: &mpsc::Sender<String>,
    ) -> Result<serde_json::Value, crate::refusal::Refusal> {
        self.schedules = Some(ListedForwarder::start(
            Arc::clone(core),
            Listed::Schedules,
            shaken,
            out.clone(),
        ));
        crate::schedules::methods::listed(core)
    }

    /// 取消订阅定时任务的推送。
    pub(crate) fn remove_schedules(&mut self) {
        self.schedules = None;
    }

    /// 订阅知识库建索引的进度（施工 W-15）：回应是空的，这一刻的样子照 `kb.list`（`indexing`、`indexed_at`）。
    pub(crate) fn add_kb(
        &mut self,
        core: &Arc<Core>,
        shaken: Shaken,
        out: &mpsc::Sender<String>,
    ) -> serde_json::Value {
        self.kb = Some(ListedForwarder::start(
            Arc::clone(core),
            Listed::Knowledge,
            shaken,
            out.clone(),
        ));
        json!({})
    }

    /// 取消订阅知识库建索引的进度：转发任务当场停。
    pub(crate) fn remove_kb(&mut self) {
        self.kb = None;
    }

    /// 订阅通知（施工 J-3 下）：回应是空的，不补以前的。
    pub(crate) fn add_notices(
        &mut self,
        core: &Arc<Core>,
        shaken: Shaken,
        out: &mpsc::Sender<String>,
    ) -> serde_json::Value {
        self.notices = Some(ListedForwarder::start(
            Arc::clone(core),
            Listed::Notices,
            shaken,
            out.clone(),
        ));
        json!({})
    }

    /// 取消订阅通知。
    pub(crate) fn remove_notices(&mut self) {
        self.notices = None;
    }

    /// 订阅包 `package` 推给人的（施工 IM-1 中下二）：回应是空的，不补以前的；一个连接能订几个包，同一个包再订
    /// 换一个新的。
    pub(crate) fn add_package_events(
        &mut self,
        core: &Arc<Core>,
        package: String,
        shaken: Shaken,
        out: &mpsc::Sender<String>,
    ) -> serde_json::Value {
        let forwarder = ListedForwarder::start(
            Arc::clone(core),
            Listed::PackageEvents(package.clone()),
            shaken,
            out.clone(),
        );
        self.package_events.insert(package, forwarder);
        json!({})
    }

    /// 取消订阅包 `package` 推给人的。
    pub(crate) fn remove_package_events(&mut self, package: &str) {
        self.package_events.remove(package);
    }
}
