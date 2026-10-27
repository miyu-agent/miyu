//! 头订阅界面块（施工 X-15 中，`widgets.md`「头」）：`subscribe {"stream": "widgets", "session"?, "showing", "page"?}`。一个连接
//! 至多一个，再订阅换一个新的。订阅时算出屏上的那几块，记成这个头在看；转发任务先写回应，再推变了的：内容变了（同一块最快每
//! 250 毫秒一次，中间的只留最新的）、包装卸开关了、程序起来停下了。读得慢、掉了队推 `resync` 停下。停了（换掉、取消、连接断了）
//! 照记下的不看了。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::broadcast::{self, error::RecvError};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use miyu_config::package::WidgetPlace;
use miyu_kernel::id::SessionId;

use super::Key;
use super::entry::{Item, declared, on_screen, one};
use crate::Core;
use crate::hello::Shaken;
use crate::refusal::Refusal;

/// 同一块两次推送至少隔多久。
const GAP: Duration = Duration::from_millis(250);

/// 头这一刻在看什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Looking {
    session: Option<String>,
    showing: Vec<WidgetPlace>,
    page: Option<(String, String)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Params {
    #[serde(rename = "stream")]
    _stream: String,
    #[serde(default)]
    session: Option<String>,
    #[serde(default)]
    showing: Vec<String>,
    #[serde(default)]
    page: Option<PageParams>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageParams {
    package: String,
    widget: String,
}

/// 读订阅的参数：会话编号写法不对、`showing` 里有不是 `home`、`sidebar`、`status` 的、多了别的格（`after` 这些），`bad_params`。
pub(crate) fn looking(params: &Value) -> Result<Looking, Refusal> {
    let params: Params = serde_json::from_value(params.clone()).map_err(|_| Refusal::BAD_PARAMS)?;
    if let Some(session) = &params.session
        && SessionId::parse(session).is_err()
    {
        return Err(Refusal::BAD_PARAMS);
    }
    let mut showing = Vec::new();
    for place in &params.showing {
        match WidgetPlace::parse(place) {
            Some(place @ (WidgetPlace::Home | WidgetPlace::Sidebar | WidgetPlace::Status)) => {
                showing.push(place);
            }
            _ => return Err(Refusal::BAD_PARAMS),
        }
    }
    Ok(Looking {
        session: params.session,
        showing,
        page: params.page.map(|page| (page.package, page.widget)),
    })
}

/// 一个头的订阅。
pub(crate) struct HeadForwarder {
    reply: Option<oneshot::Sender<String>>,
    task: JoinHandle<()>,
    /// 这个头在看的：停的时候照它不看了。
    keys: Arc<Mutex<BTreeSet<Key>>>,
    core: Arc<Core>,
}

impl std::fmt::Debug for HeadForwarder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HeadForwarder")
    }
}

impl HeadForwarder {
    /// 起一个：先拿收变化的几头，再算这一刻的块、记成在看；交回它和订阅的回应 `{"widgets": […]}`（经它写出去）。
    pub(crate) fn start(
        core: Arc<Core>,
        looking: Looking,
        shaken: Shaken,
        out: mpsc::Sender<String>,
    ) -> (HeadForwarder, Value) {
        let receivers = (
            core.widgets.changes(),
            core.package_changes.subscribe(),
            core.extensions.subscribe(),
        );
        let items = visible(&core, &looking, None, shaken.whose());
        let keys = watched(&items, &looking);
        core.widgets.look(&keys);
        let language = shaken.now(&core).language;
        let listed: Vec<Value> = items
            .iter()
            .map(|item| entry(&core, item, &looking, language))
            .collect();
        let keys = Arc::new(Mutex::new(keys));
        let (reply, answer) = oneshot::channel();
        let follow = Follow {
            core: Arc::clone(&core),
            looking,
            shaken,
            items,
            keys: Arc::clone(&keys),
            last: BTreeMap::new(),
            due: BTreeMap::new(),
        };
        let task = tokio::spawn(follow.run(receivers, answer, out));
        let forwarder = HeadForwarder {
            reply: Some(reply),
            task,
            keys,
            core,
        };
        (forwarder, json!({ "widgets": listed }))
    }

    /// 交订阅的回应；已经交过、它已经不收了的，交回这一行。
    pub(crate) fn reply(&mut self, line: String) -> Result<(), String> {
        match self.reply.take() {
            Some(reply) => reply.send(line),
            None => Err(line),
        }
    }
}

impl Drop for HeadForwarder {
    fn drop(&mut self) {
        self.task.abort();
        let keys = std::mem::take(&mut *self.keys.lock().unwrap_or_else(PoisonError::into_inner));
        self.core.widgets.leave(&keys);
    }
}

/// `viewer` 的头看得到的块：没带会话的不列跟着会话的；只要包 `only` 的就写上它。
fn visible(
    core: &Core,
    looking: &Looking,
    only: Option<&str>,
    viewer: &miyu_kernel::id::AccountId,
) -> Vec<Item> {
    declared(core, only, viewer)
        .into_iter()
        .filter(|item| !item.widget.per_session || looking.session.is_some())
        .collect()
}

/// 屏上的那几块：头在看它们。
fn watched(items: &[Item], looking: &Looking) -> BTreeSet<Key> {
    items
        .iter()
        .filter(|item| on_screen(item, &looking.showing, looking.page.as_ref()))
        .map(|item| item.key(looking.session.as_deref()))
        .collect()
}

fn entry(core: &Core, item: &Item, looking: &Looking, language: &str) -> Value {
    let content = on_screen(item, &looking.showing, looking.page.as_ref());
    one(core, item, looking.session.as_deref(), content, language)
}

/// 转发任务手里的。
struct Follow {
    core: Arc<Core>,
    looking: Looking,
    shaken: Shaken,
    items: Vec<Item>,
    keys: Arc<Mutex<BTreeSet<Key>>>,
    /// 每一块上次推的时候。
    last: BTreeMap<(String, String), Instant>,
    /// 等着推的块和到点的时候。
    due: BTreeMap<(String, String), Instant>,
}

impl Follow {
    async fn run(
        mut self,
        (mut changed, mut packages, mut extensions): (
            broadcast::Receiver<Key>,
            broadcast::Receiver<String>,
            broadcast::Receiver<String>,
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
            let next = self.due.values().min().copied();
            let lines = tokio::select! {
                got = changed.recv() => match got {
                    Ok(key) => self.content_changed(&key),
                    Err(RecvError::Lagged(_)) => break resync(&out).await,
                    Err(RecvError::Closed) => break,
                },
                got = packages.recv() => match got {
                    Ok(package) => self.package_changed(&package),
                    Err(RecvError::Lagged(_)) => break resync(&out).await,
                    Err(RecvError::Closed) => break,
                },
                got = extensions.recv() => match got {
                    Ok(package) => self.package_changed(&package),
                    Err(RecvError::Lagged(_)) => break resync(&out).await,
                    Err(RecvError::Closed) => break,
                },
                () = sleep(next) => self.ripe(),
            };
            for line in lines {
                if out.send(line).await.is_err() {
                    return;
                }
            }
        }
    }

    /// 一块的内容变了：这个头看得到的，到点了就推，没到点的等着。
    fn content_changed(&mut self, key: &Key) -> Vec<String> {
        let mine = self.items.iter().any(|item| {
            item.package == key.package
                && item.widget.id == key.widget
                && item.owner == key.account
                && (key.session.is_none() || key.session == self.looking.session)
        });
        if !mine {
            return Vec::new();
        }
        let id = (key.package.clone(), key.widget.clone());
        let now = Instant::now();
        match self.last.get(&id) {
            Some(last) if now < *last + GAP => {
                self.due.entry(id).or_insert(*last + GAP);
                Vec::new()
            }
            _ => self.push(&id, now).into_iter().collect(),
        }
    }

    /// 到点了的推出去。
    fn ripe(&mut self) -> Vec<String> {
        let now = Instant::now();
        let ready: Vec<(String, String)> = self
            .due
            .iter()
            .filter(|(_, at)| **at <= now)
            .map(|(id, _)| id.clone())
            .collect();
        ready.iter().filter_map(|id| self.push(id, now)).collect()
    }

    fn push(&mut self, id: &(String, String), now: Instant) -> Option<String> {
        self.due.remove(id);
        self.last.insert(id.clone(), now);
        let item = self
            .items
            .iter()
            .find(|item| item.package == id.0 && item.widget.id == id.1)?;
        let language = self.shaken.now(&self.core).language;
        Some(changed_line(
            &id.0,
            &id.1,
            Some(entry(&self.core, item, &self.looking, language)),
        ))
    }

    /// 包装卸、开关、程序起停了：照这一刻重算它的块，没了的推 `null`、不看了，新的记成在看，别的整块推一遍。
    fn package_changed(&mut self, package: &str) -> Vec<String> {
        let now = visible(
            &self.core,
            &self.looking,
            Some(package),
            self.shaken.whose(),
        );
        let before: Vec<Item> = self
            .items
            .iter()
            .filter(|item| item.package == package)
            .cloned()
            .collect();
        let old_keys = watched(&before, &self.looking);
        let new_keys = watched(&now, &self.looking);
        let gone: BTreeSet<Key> = old_keys.difference(&new_keys).cloned().collect();
        let added: BTreeSet<Key> = new_keys.difference(&old_keys).cloned().collect();
        self.core.widgets.look(&added);
        self.core.widgets.leave(&gone);
        {
            let mut keys = self.keys.lock().unwrap_or_else(PoisonError::into_inner);
            keys.retain(|key| !gone.contains(key));
            keys.extend(added);
        }
        let language = self.shaken.now(&self.core).language;
        let mut lines: Vec<String> = before
            .iter()
            .filter(|old| !now.iter().any(|item| item.widget.id == old.widget.id))
            .map(|old| changed_line(package, &old.widget.id, None))
            .collect();
        lines.extend(now.iter().map(|item| {
            changed_line(
                package,
                &item.widget.id,
                Some(entry(&self.core, item, &self.looking, language)),
            )
        }));
        self.items.retain(|item| item.package != package);
        self.items.extend(now);
        lines
    }
}

fn changed_line(package: &str, id: &str, widget: Option<Value>) -> String {
    json!({"jsonrpc": "2.0", "method": "widget.changed", "params": {"package": package, "id": id, "widget": widget}})
        .to_string()
}

async fn resync(out: &mpsc::Sender<String>) {
    tracing::warn!(target: "miyu::endpoint", stream = "widgets", "lagged, resync");
    let line = json!({"jsonrpc": "2.0", "method": "resync", "params": {"stream": "widgets"}});
    if out.send(line.to_string()).await.is_err() {
        // 连接断了：没人收。
    }
}

/// 等到 `at`；没有等着推的永远等着。
async fn sleep(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}
