//! 封存（施工 C-8 下，`docs/blueprint/session/fork.md`「封存」）：`session.sealed` 只在两轮之间记、只记一次；封存以后不再开轮。
//! 人生结束（施工 LF-4 上，`life.md`「结束」）：`life.ended` 只记一次；记了以后同封存，不再开轮。

use super::Ledger;
use crate::event::{Body, Event};
use crate::id::TurnId;

/// 封存了没有：日志里有过 `session.sealed`；是不是人生会话、人生结束了没有：日志里有过 `life.started`、`life.ended`。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Sealing {
    sealed: bool,
    life_started: bool,
    life_ended: bool,
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
            (Body::LifeEnded(_), _) if self.life_ended => {
                Err("the life has already ended".to_string())
            }
            (Body::TurnStarted(_), _) if self.life_ended => {
                Err("an ended life starts no turn".to_string())
            }
            _ => Ok(()),
        }
    }

    /// 记下追加的一条。
    pub(super) fn record(&mut self, event: &Event) {
        match event.body {
            Body::SessionSealed(_) => self.sealed = true,
            Body::LifeStarted(_) => self.life_started = true,
            Body::LifeEnded(_) => self.life_ended = true,
            _ => {}
        }
    }
}

impl Ledger {
    /// 封存了没有（施工 C-8 下）：封存了的不再开轮，改对话的命令拒。
    pub fn sealed(&self) -> bool {
        self.sealing.sealed
    }

    /// 人生结束了没有（施工 LF-4 上）：日志里有过 `life.ended`。
    pub fn life_ended(&self) -> bool {
        self.sealing.life_ended
    }

    /// 是不是人生会话（施工 LF-4 下）：日志里有过 `life.started`，结束了的也是。
    pub fn life_started(&self) -> bool {
        self.sealing.life_started
    }

    /// 封存了，或者人生结束了（施工 LF-4 上）：改对话的命令不收，不再开轮。结束了、这一轮还没跑完的，这一轮结束时封存。
    pub fn closed(&self) -> bool {
        self.sealing.sealed || self.sealing.life_ended
    }

    /// 有一轮正在进行。
    pub(crate) fn turn_open(&self) -> bool {
        self.open.is_some()
    }
}
