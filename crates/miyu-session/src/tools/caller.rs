//! 调用的事实（施工 X-43，`docs/blueprint/skills.md`「脚本」第 2 条）：回合开始照这一轮的 `speakers`（施工 E-0）算一次开口的是不是
//! 机器主人，记在这一轮上；派工具（直接调的、脚本里调的）时照派它的那一轮交 [`Caller`]。内核不动。拿不到的照最严：回合中途核心
//! 重启、载入以后接着干的那一轮由那一条结束开，没有人开口，场所会话里不是主人。

use miyu_kernel::facts::Speaker;
use miyu_kernel::id::{TurnId, VenueId};
use miyu_tool::Caller;

use crate::agents::LOCAL;

/// 一个会话的：它的场所，最近开始的那一轮和那一轮开口的是不是主人。
pub(crate) struct Callers {
    venue: VenueId,
    round: Option<(TurnId, bool)>,
}

impl Callers {
    /// 场所是 `venue` 的会话：还没有开始的一轮。
    pub(crate) fn new(venue: VenueId) -> Callers {
        Callers { venue, round: None }
    }

    /// 回合 `turn` 开始了，开口的是 `speakers`。
    pub(crate) fn start(&mut self, turn: TurnId, speakers: &[Speaker]) {
        self.round = Some((turn, owner(speakers)));
    }

    /// 派在 `turn` 这一轮的调用的事实：本机的会话是主人；场所会话照记下的那一轮，不是这一轮的、没记下的不是。
    pub(crate) fn of(&self, turn: Option<TurnId>) -> Caller {
        let owner = self.venue.as_str() == LOCAL
            || matches!(self.round, Some((round, true)) if Some(round) == turn);
        Caller {
            venue: self.venue.clone(),
            owner,
        }
    }
}

impl super::Tools {
    /// 回合 `turn` 开始了，开口的是 `speakers`：记下这一轮开口的是不是主人。
    pub(crate) fn round_started(&mut self, turn: TurnId, speakers: &[Speaker]) {
        self.callers.start(turn, speakers);
    }
}

/// 这一轮开口的是不是主人：有人开口、每个都是才是（群里几个人有一个不是的就不是，宁严）。
fn owner(speakers: &[Speaker]) -> bool {
    !speakers.is_empty() && speakers.iter().all(|speaker| speaker.owner)
}

#[cfg(test)]
mod tests;
