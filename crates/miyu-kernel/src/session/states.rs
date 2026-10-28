//! 包的会话状态（施工 TW-1，设计 `46-包的会话状态.md`）：当前的由有效历史另记的那张表算（`history/states.rs`），这里管程序
//! 直接写的那一条、交给头的那一份。
//!
//! - 变了才推：落了盘、推完事件以后，哪个包给头看的那几格和上次告诉头的不一样，推一条瞬时的 `package.state_changed`。写了、
//!   清了、撤销、恢复、清空上下文都走这一条。
//! - 先落盘，后推送：还没落盘的事件里有动状态的，等它们落了盘再比。
//! - 造会话时告诉过头的是空的；载入时当已经告诉过当前那份（头订阅时从回应里拿）。

use std::collections::BTreeMap;

use super::Session;
use super::action::Action;
use crate::event::{Body, PackageState, PackageStateChanged, State, Transient, TransientBody};
use crate::history::touches_states;
use crate::id::CommandId;
use crate::origin::By;
use crate::time::Timestamp;

/// 上次告诉头的：每个包给头看的那几格。只在内存里。
pub(super) type Told = BTreeMap<String, State>;

impl Session {
    /// 每个包当前的状态，照包的编号排：清掉了的不在里面。
    pub fn package_states(&self) -> BTreeMap<String, State> {
        self.history.package_states()
    }

    /// 包的程序直接换上它的一份（`session.set_state`）：回合进行中的带上这个回合；任何时候都收，不开回合。落了盘回应。
    pub(super) fn set_package_state(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        written: PackageState,
    ) -> Vec<Action> {
        let event = self.record(at, by, Some(id.clone()), Body::PackageState(written));
        self.accept(id, vec![event.seq]);
        vec![Action::Append(vec![event])]
    }

    /// 落了盘以后：给头看的那几格变了的包，各推一条 `package.state_changed`，记下这一份。还有没落盘的事件会动状态的，先不推。
    pub(super) fn states_changed(&mut self, at: Timestamp) -> Vec<Action> {
        if self.unstored.iter().any(touches_states) {
            return Vec::new();
        }
        let now: Told = self
            .package_states()
            .into_iter()
            .map(|(package, state)| (package, state.shown()))
            .filter(|(_, shown)| !shown.is_empty())
            .collect();
        let mut packages: Vec<&String> = now.keys().chain(self.told_states.keys()).collect();
        packages.sort();
        packages.dedup();
        let actions = packages
            .into_iter()
            .filter(|package| now.get(*package) != self.told_states.get(*package))
            .map(|package| {
                Action::PushTransient(Transient {
                    at,
                    turn: self.turn.as_ref().map(|turn| turn.id),
                    by: By::Kernel,
                    cause: None,
                    body: TransientBody::PackageStateChanged(PackageStateChanged {
                        package: package.clone(),
                        state: now.get(package).cloned().unwrap_or_default(),
                    }),
                })
            })
            .collect();
        self.told_states = now;
        actions
    }

    /// 载入以后：当已经告诉过头当前那份。
    pub(super) fn told_current_states(&mut self) {
        self.told_states = self
            .package_states()
            .into_iter()
            .map(|(package, state)| (package, state.shown()))
            .filter(|(_, shown)| !shown.is_empty())
            .collect();
    }
}
