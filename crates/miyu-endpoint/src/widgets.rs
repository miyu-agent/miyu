//! 界面块（施工 X-15 中，`docs/blueprint/widgets.md`，设计 `33-界面扩展.md`）：程序推来的内容记在这里（只在内存里），头订阅
//! 时照这里算；谁在看也记在这里，一块从没人看变成有人看、反过来，广播给那个包的程序的连接（`widget.watch`、`widget.unwatch`）。
//! 跟着会话的块只记有人在看的会话的；程序停了，它的内容全清（[`Widgets::forget`]）。
//!
//! 程序推内容的两个方法在 `widgets/set.rs`，头的订阅在 `widgets/head.rs`，推给头的一块怎么写在 `widgets/entry.rs`。

mod entry;
pub(crate) mod head;
pub(crate) mod set;
pub(crate) mod watch;

use std::collections::{BTreeSet, HashMap};
use std::sync::{Mutex, PoisonError};

use serde_json::Value;
use tokio::sync::broadcast;

use miyu_kernel::id::AccountId;

use crate::extensions::Instance;

/// 一块：哪个包的哪一块，跟着会话的带会话的编号。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Key {
    /// 包的编号。
    pub(crate) package: String,
    /// 清单里这一块的编号。
    pub(crate) widget: String,
    /// 跟着会话的：哪个会话。
    pub(crate) session: Option<String>,
    /// 每人一份的包（施工 IM-1 中下五）：哪个人那一份的块；一份的包没有。
    pub(crate) account: Option<AccountId>,
}

impl Key {
    /// 这一块归 `instance` 那一份：包对得上，每人一份的人也对得上。
    pub(crate) fn of(&self, instance: &Instance) -> bool {
        self.package == instance.package && self.account == instance.account
    }
}

/// 程序推来的、这一刻的一份。
#[derive(Debug, Clone)]
pub(crate) struct Shown {
    /// 查过的内容，字还没照语言挑。
    pub(crate) content: Value,
    /// 换掉的标题；没换的没有。
    pub(crate) title: Option<Value>,
}

/// 一块有人开始看了（`on`）、没人看了。
#[derive(Debug, Clone)]
pub(crate) struct Watch {
    /// 哪一块。
    pub(crate) key: Key,
    /// 有人开始看了。
    pub(crate) on: bool,
}

/// 记内容、记谁在看。
pub(crate) struct Widgets {
    state: Mutex<State>,
    /// 哪一块的内容变了：头的订阅照它推。
    changed: broadcast::Sender<Key>,
    /// 谁开始看、不看了：程序的连接照它说。
    watches: broadcast::Sender<Watch>,
}

#[derive(Default)]
struct State {
    shown: HashMap<Key, Shown>,
    watchers: HashMap<Key, usize>,
}

impl Default for Widgets {
    fn default() -> Widgets {
        Widgets {
            state: Mutex::default(),
            changed: broadcast::channel(256).0,
            watches: broadcast::channel(256).0,
        }
    }
}

impl Widgets {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 收「哪一块的内容变了」。
    pub(crate) fn changes(&self) -> broadcast::Receiver<Key> {
        self.changed.subscribe()
    }

    /// 收「谁开始看、不看了」。
    pub(crate) fn watch_changes(&self) -> broadcast::Receiver<Watch> {
        self.watches.subscribe()
    }

    /// 这一刻有人在看的、`instance` 那一份的那几块。
    pub(crate) fn watched(&self, instance: &Instance) -> BTreeSet<Key> {
        self.state()
            .watchers
            .keys()
            .filter(|key| key.of(instance))
            .cloned()
            .collect()
    }

    /// 一个头开始看 `keys`：从没人看变成有人看的，广播一声。
    pub(crate) fn look(&self, keys: &BTreeSet<Key>) {
        let mut state = self.state();
        for key in keys {
            let count = state.watchers.entry(key.clone()).or_insert(0);
            *count += 1;
            if *count == 1 {
                self.tell(key, true);
            }
        }
    }

    /// 一个头不看 `keys` 了：最后一个人也不看的，广播一声；跟着会话的内容扔掉。
    pub(crate) fn leave(&self, keys: &BTreeSet<Key>) {
        let mut state = self.state();
        for key in keys {
            let Some(count) = state.watchers.get_mut(key) else {
                continue;
            };
            *count -= 1;
            if *count == 0 {
                state.watchers.remove(key);
                if key.session.is_some() {
                    state.shown.remove(key);
                }
                self.tell(key, false);
            }
        }
    }

    /// 这一块这一刻的一份；程序还没推、清掉了的没有。
    pub(crate) fn get(&self, key: &Key) -> Option<Shown> {
        self.state().shown.get(key).cloned()
    }

    /// 记下程序推来的一份，广播变了。跟着会话的、这个会话没人在看的不留，交回 `false`。
    pub(crate) fn put(&self, key: Key, shown: Shown) -> bool {
        {
            let mut state = self.state();
            if key.session.is_some() && !state.watchers.contains_key(&key) {
                return false;
            }
            state.shown.insert(key.clone(), shown);
        }
        self.changed(key);
        true
    }

    /// 清掉一块，广播变了。
    pub(crate) fn clear(&self, key: Key) {
        let had = self.state().shown.remove(&key).is_some();
        if had {
            self.changed(key);
        }
    }

    /// `instance` 那一份的程序停了：它的内容全清，清掉的各广播一声。别人那一份的不动（施工 IM-1 中下五）。
    pub(crate) fn forget(&self, instance: &Instance) {
        let gone: Vec<Key> = {
            let mut state = self.state();
            let gone: Vec<Key> = state
                .shown
                .keys()
                .filter(|key| key.of(instance))
                .cloned()
                .collect();
            for key in &gone {
                state.shown.remove(key);
            }
            gone
        };
        for key in gone {
            self.changed(key);
        }
    }

    fn changed(&self, key: Key) {
        if self.changed.send(key).is_err() {
            // 没有订阅着的头：不用说。
        }
    }

    fn tell(&self, key: &Key, on: bool) {
        let watch = Watch {
            key: key.clone(),
            on,
        };
        if self.watches.send(watch).is_err() {
            // 程序没连着：它连上时照 [`Widgets::watched`] 补。
        }
    }
}

#[cfg(test)]
mod tests;
