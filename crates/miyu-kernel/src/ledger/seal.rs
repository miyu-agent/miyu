//! 封存（施工 C-8 下，`docs/blueprint/session/fork.md`「封存」）：`session.sealed` 只在两轮之间记、只记一次；封存以后不再开轮。

use super::Ledger;
use crate::event::{Body, Event};
use crate::id::TurnId;

/// 封存了没有：日志里有过 `session.sealed`。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Sealing {
    sealed: bool,
}

impl Sealing {
    /// 照封存的规矩查 `event`：`open` 是正在进行的回合。
    pub(super) fn check(&self, open: Option<TurnId>, event: &Event) -> Result<(), String> {
        match (&event.body, open) {
            (Body::SessionSealed(_), _) if self.sealed => {
                Err("the session is already sealed".to_string())
            }
            (Body::SessionSealed(_), Some(open)) => Err(format!(
                "turn {open} is still running; a session is sealed only between turns"
            )),
            (Body::TurnStarted(_), _) if self.sealed => {
                Err("a sealed session starts no turn".to_string())
            }
            _ => Ok(()),
        }
    }

    /// 记下追加的一条。
    pub(super) fn record(&mut self, event: &Event) {
        if matches!(event.body, Body::SessionSealed(_)) {
            self.sealed = true;
        }
    }
}

impl Ledger {
    /// 封存了没有（施工 C-8 下）：封存了的不再开轮，改对话的命令拒。
    pub fn sealed(&self) -> bool {
        self.sealing.sealed
    }
}
