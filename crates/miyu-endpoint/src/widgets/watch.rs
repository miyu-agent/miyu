//! 告诉程序谁在看（施工 X-15 中，`widgets.md`「核心告诉程序」）：核心拉起的扩展握了手，照这一刻有人在看的那几块各推一条
//! `widget.watch`，之后一块从没人看变成有人看推 `widget.watch`，最后一个人也不看了推 `widget.unwatch`。掉了队照这一刻重对一遍。
//! 连接断了（程序停了）它的内容全清。

use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::json;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::Key;
use crate::Core;
use crate::extensions::Instance;

/// 推给一个程序的任务。
pub(crate) struct Watching {
    task: JoinHandle<()>,
    core: Arc<Core>,
    instance: Instance,
}

impl std::fmt::Debug for Watching {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Watching")
    }
}

impl Watching {
    /// 起一个：听包 `instance` 的块，写进 `out`。
    pub(crate) fn start(
        core: Arc<Core>,
        instance: Instance,
        out: mpsc::Sender<String>,
    ) -> Watching {
        let task = tokio::spawn(tell(Arc::clone(&core), instance.clone(), out));
        Watching {
            task,
            core,
            instance,
        }
    }
}

impl Drop for Watching {
    fn drop(&mut self) {
        self.task.abort();
        self.core.widgets.forget(&self.instance);
    }
}

async fn tell(core: Arc<Core>, instance: Instance, out: mpsc::Sender<String>) {
    let mut changes = core.widgets.watch_changes();
    let mut told = BTreeSet::new();
    if !catch_up(&core, &instance, &mut told, &out).await {
        return;
    }
    loop {
        match changes.recv().await {
            Ok(watch) if watch.key.of(&instance) => {
                let fresh = match watch.on {
                    true => told.insert(watch.key.clone()),
                    false => told.remove(&watch.key),
                };
                if fresh && out.send(line(&watch.key, watch.on)).await.is_err() {
                    return;
                }
            }
            Ok(_) => {}
            Err(RecvError::Lagged(_)) => {
                if !catch_up(&core, &instance, &mut told, &out).await {
                    return;
                }
            }
            Err(RecvError::Closed) => return,
        }
    }
}

/// 照这一刻有人在看的对一遍：说过、现在没人看的推 `widget.unwatch`，新的推 `widget.watch`。连接断了交回 `false`。
async fn catch_up(
    core: &Core,
    instance: &Instance,
    told: &mut BTreeSet<Key>,
    out: &mpsc::Sender<String>,
) -> bool {
    let now = core.widgets.watched(instance);
    let lines: Vec<String> = told
        .difference(&now)
        .map(|key| line(key, false))
        .chain(now.difference(told).map(|key| line(key, true)))
        .collect();
    *told = now;
    for line in lines {
        if out.send(line).await.is_err() {
            return false;
        }
    }
    true
}

fn line(key: &Key, on: bool) -> String {
    let method = if on { "widget.watch" } else { "widget.unwatch" };
    let mut params = json!({ "widget": key.widget });
    if let Some(session) = &key.session {
        params["session"] = json!(session);
    }
    json!({"jsonrpc": "2.0", "method": method, "params": params}).to_string()
}
