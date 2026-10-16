//! 主线投影里的支线（施工 O-36，`onebot.md` 第一条「群里怎么叫她」第 2 条「支线」、第 6、7 条；`session/fork.md`）：纯逻辑。
//!
//! - 分叉记一条（[`Projection::forked`]）：子会话编号、分叉点 `upto`、分派给它的那几条；还没开轮的照它们算回的人。
//! - 序号对照表：支线的序号从 `upto + 1` 数起，会和主线后来的序号撞上，不能直接收进主线的投影（施工单「要拍板的」第 1 条）。
//!   `upto` 以内的两边一样（子会话的底子就是主线到那一点为止的日志）；抄进去的一条一对（[`Projection::copied`]）。
//! - 支线推来的事件（[`Projection::lane_take`]）：触发换回主线序号，再照主线那一套算（`track.rs`），交回调的一方要做的
//!   （[`Seen`]：表情记进了哪一轮、摘，她的话照主线发，封存了退订）。
//! - 分派要的 `Lines`（[`Projection::lines`]）、开轮带的事实指哪几条（[`Projection::answering`]）。
//! - 抄进支线用的人话（[`Kept`]）：推来的 `message.user` 当场写成 `session.send` 的参数留着（施工单「要拍板的」第 2 条），留到
//!   用不着为止：比主线在跑的这一轮的开始、各条支线的分叉点里最早的那一个晚的留着，主线闲着、又没有支线的一条不留
//!   （[`Projection::prune`]）。

use std::collections::BTreeMap;

use miyu_chat::{Line, Lines};
use miyu_client::events::Block;
use miyu_client::events::By;
use miyu_client::events::{Body, Event};
use miyu_client::ids::{ExternalId, Seq};
use serde_json::{Value, json};

use super::track::Track;
use super::{Projection, Speaking};

/// 一条支线。
#[derive(Debug)]
pub(super) struct Lane {
    /// 子会话编号。
    pub(super) session: String,
    /// 分叉点：主线到这一条为止是它的底子。
    upto: u64,
    /// 收过的最后一条（支线自己的序号）：重的、更早的不收。
    last: u64,
    /// 抄进去的：支线序号 → 主线序号。
    copies: BTreeMap<u64, u64>,
    /// 这条线这一轮的样子。
    pub(super) track: Track,
    /// 分派给它的几条（主线序号）：还没开轮时照它们算回的人、带事实。
    asked: Vec<u64>,
    /// 开过轮了。
    started: bool,
}

/// 抄进支线用的一条人话（`onebot.md`「群里怎么叫她」第 7 条「抄」）。
#[derive(Debug, Clone)]
pub(super) struct Kept {
    /// 原来的命令编号（`cause`）。
    cause: String,
    /// `session.send` 的参数（不带 `session`）：字、`as`、带了 `ambient` 的 `venue`、图和文件。
    params: Value,
}

/// 要抄进支线的一条。
#[derive(Debug, Clone, PartialEq)]
pub(in crate::core::route) struct Carry {
    /// 主线序号。
    pub(in crate::core::route) msg: u64,
    /// 命令编号：原来的加 `/branch`（`chat.md` 第七条第 1 条：不然会被当成同一条去重吞掉）。
    pub(in crate::core::route) id: String,
    /// `session.send` 的参数，不带 `session`。
    pub(in crate::core::route) params: Value,
}

/// 支线推来的一条收进来以后，调的一方还要做的。
#[derive(Debug, PartialEq, Eq)]
pub(in crate::core::route) enum Seen {
    /// 没什么要做的。
    Nothing,
    /// 这几条（主线序号）进了这条线回合编号是 `turn` 的那一轮：贴着的表情记下进了哪一轮。
    Took {
        /// 回合编号。
        turn: u64,
        /// 进了这一轮的几条，主线序号。
        msgs: Vec<u64>,
    },
    /// 回合编号是这个的那一轮完了：摘它收了的表情。
    Ended(u64),
    /// 她说了一段：照主线的出站链发。
    Spoke(Speaking),
    /// 封存了：已经从投影里去掉，退订。
    Sealed,
}

impl Projection {
    /// 分叉出了子会话 `lane`，分叉点是 `upto`，分派给它的是 `asked`（主线序号）。
    pub(in crate::core::route) fn forked(&mut self, lane: &str, upto: u64, asked: &[u64]) {
        self.lanes.push(Lane {
            session: lane.to_string(),
            upto,
            last: upto,
            copies: BTreeMap::new(),
            track: Track::default(),
            asked: asked.to_vec(),
            started: false,
        });
        self.prune();
    }

    /// 主线序号是 `msg` 的那一条抄进了支线 `lane`，在那里的序号是 `at`。
    pub(in crate::core::route) fn copied(&mut self, lane: &str, at: u64, msg: u64) {
        if let Some(lane) = self.lane_mut(lane) {
            lane.copies.insert(at, msg);
        }
    }

    /// 支线 `lane` 还要抄进去的人话，照先后：留着的里面比分叉点晚、还没抄过的；`only` 有的只要其中这几条（主线序号）。
    pub(in crate::core::route) fn to_copy(&self, lane: &str, only: Option<&[u64]>) -> Vec<Carry> {
        let Some(lane) = self.lane(lane) else {
            return Vec::new();
        };
        self.kept
            .range(lane.upto + 1..)
            .filter(|(seq, _)| !lane.copies.values().any(|copied| copied == *seq))
            .filter(|(seq, _)| only.is_none_or(|only| only.contains(seq)))
            .map(|(seq, kept)| Carry {
                msg: *seq,
                id: format!("{}/branch", kept.cause),
                params: kept.params.clone(),
            })
            .collect()
    }

    /// 主线序号是 `msgs` 的几条在支线 `lane` 里的序号（`session.respond` 的 `to`）：分叉点以内的照原样，抄进去的照对照表；没抄
    /// 进去的不要。
    pub(in crate::core::route) fn targets(&self, lane: &str, msgs: &[u64]) -> Vec<u64> {
        let Some(lane) = self.lane(lane) else {
            return Vec::new();
        };
        msgs.iter()
            .filter_map(|msg| match *msg <= lane.upto {
                true => Some(*msg),
                false => lane
                    .copies
                    .iter()
                    .find(|(_, copied)| *copied == msg)
                    .map(|(at, _)| *at),
            })
            .collect()
    }

    /// 支线 `lane` 推来的一条 `event`：换回主线序号收进来，交回调的一方还要做的。不认识的支线、重的、更早的什么都不做。
    pub(in crate::core::route) fn lane_take(&mut self, lane: &str, event: &Event) -> Seen {
        let seq = event.seq.get();
        let Some(index) = self.lanes.iter().position(|one| one.session == lane) else {
            return Seen::Nothing;
        };
        if seq <= self.lanes[index].last {
            return Seen::Nothing;
        }
        self.lanes[index].last = seq;
        let turn = event.turn.map(|turn| turn.started().get());
        match &event.body {
            Body::TurnStarted(started) => {
                let one = &self.lanes[index];
                let triggers = one
                    .track
                    .triggers(one.main_seqs(&started.triggers), started.trigger);
                self.lanes[index].started = true;
                self.started(lane, &triggers, turn, event.at);
                took(turn, &triggers)
            }
            Body::TurnJoined(joined) => {
                let triggers = self.lanes[index].main_seqs(&joined.triggers);
                self.joined(lane, seq, &triggers, turn);
                took(turn, &triggers)
            }
            Body::TurnEnded(_) => {
                self.ended(lane, turn);
                turn.map_or(Seen::Nothing, Seen::Ended)
            }
            Body::MessageAssistant(_) => turn.map_or(Seen::Nothing, |turn| {
                Seen::Spoke(self.speaking(lane, turn, false))
            }),
            Body::SessionSealed(_) => {
                self.lanes.remove(index);
                self.prune();
                Seen::Sealed
            }
            _ => Seen::Nothing,
        }
    }

    /// 分派要的几条线（`chat.md` 第四条）：主线在跑的是 `Busy`，回的人照这一轮；支线同理，还没开轮的回的是分派给它的人，这一轮
    /// 完了、还没封存的是 `Idle`（照样占一个位置）。最多几条支线是 `parallel`。
    pub(in crate::core::route) fn lines(&self, parallel: u8) -> Lines {
        let busy = |targets: Vec<ExternalId>| Line::Busy { targets };
        let main = self
            .main
            .running
            .as_ref()
            .map_or(Line::Idle, |running| busy(running.to.clone()));
        let lanes = self
            .lanes
            .iter()
            .map(|lane| match (&lane.track.running, lane.started) {
                (Some(running), _) => busy(running.to.clone()),
                (None, false) => {
                    let mut to = Vec::new();
                    let asked = lane.asked.iter().filter_map(|seq| self.said.get(seq));
                    super::add(&mut to, asked.map(|speaker| &speaker.id));
                    busy(to)
                }
                (None, true) => Line::Idle,
            })
            .collect();
        Lines {
            main,
            lanes,
            parallel,
        }
    }

    /// 第 `index` 条支线（分派的 `JoinLane`）的子会话编号。
    pub(in crate::core::route) fn lane_at(&self, index: usize) -> Option<&str> {
        self.lanes.get(index).map(|lane| lane.session.as_str())
    }

    /// 正在跑的支线的子会话编号，照分叉的先后。
    pub(in crate::core::route) fn lanes(&self) -> Vec<String> {
        self.lanes.iter().map(|lane| lane.session.clone()).collect()
    }

    /// 子会话 `lane` 是不是这个群正在跑的一条支线。
    pub(in crate::core::route) fn has_lane(&self, lane: &str) -> bool {
        self.lane(lane).is_some()
    }

    /// 支线 `lane` 收到的最后一条的序号（掉了队照它再订阅）；不认识的是空的。
    pub(in crate::core::route) fn lane_last(&self, lane: &str) -> Option<u64> {
        self.lane(lane).map(|lane| lane.last)
    }

    /// 支线 `lane` 用不成了（订阅不上、开不了轮）：不等封存，当场去掉。
    pub(in crate::core::route) fn dropped(&mut self, lane: &str) {
        self.lanes.retain(|one| one.session != lane);
        self.prune();
    }

    /// 序号是 `seq` 的人话是谁说的（分派看这个人）；没收过的是空的。
    pub(in crate::core::route) fn sender(&self, seq: u64) -> Option<&ExternalId> {
        self.said.get(&seq).map(|speaker| &speaker.id)
    }

    /// 开轮带的事实指哪几条，平台编号照先后（「群里怎么叫她」第 7 条「事实」）：`lanes` 是真的，各条支线这一轮在回的（还没开轮的
    /// 照分派给它的）；假的，主线这一轮在回的。没有平台编号的不要。
    pub(in crate::core::route) fn answering(&self, lanes: bool) -> Vec<String> {
        let msgs: Vec<u64> = match lanes {
            true => self
                .lanes
                .iter()
                .flat_map(|lane| match &lane.track.running {
                    Some(running) => running.msgs.clone(),
                    None if !lane.started => lane.asked.clone(),
                    None => Vec::new(),
                })
                .collect(),
            false => self
                .main
                .running
                .as_ref()
                .map(|running| running.msgs.clone())
                .unwrap_or_default(),
        };
        msgs.iter()
            .filter_map(|seq| self.msg(*seq).map(str::to_string))
            .collect()
    }

    /// 序号是 `seq` 的人话 `event` 留着抄进支线用（见模块开头）：主线闲着、又没有支线的不留；写不成参数的（不是外部身份说的、没有
    /// 命令编号、没有场所的格，照说不会）不留。
    pub(super) fn keep(&mut self, seq: u64, event: &Event) {
        if self.floor().is_none_or(|floor| seq <= floor) {
            return;
        }
        if let Some(kept) = kept(event) {
            self.kept.insert(seq, kept);
        }
    }

    /// 用不着的人话不再留着：比主线在跑的这一轮的开始、各条支线的分叉点里最早的那一个晚的留着，都没有的一条不留。
    pub(super) fn prune(&mut self) {
        match self.floor() {
            Some(floor) => self.kept.retain(|seq, _| *seq > floor),
            None => self.kept.clear(),
        }
    }

    /// 留着的界：主线在跑的这一轮的回合编号、各条支线的分叉点里最早的那一个；都没有的是空的。
    fn floor(&self) -> Option<u64> {
        let main = self.main.running.as_ref().map(|running| running.turn);
        main.into_iter()
            .chain(self.lanes.iter().map(|lane| lane.upto))
            .min()
    }

    /// 还在的支线 `lane`。
    fn lane(&self, lane: &str) -> Option<&Lane> {
        self.lanes.iter().find(|one| one.session == lane)
    }

    /// 同 [`Projection::lane`]，改得了。
    fn lane_mut(&mut self, lane: &str) -> Option<&mut Lane> {
        self.lanes.iter_mut().find(|one| one.session == lane)
    }
}

impl Lane {
    /// 支线里的序号 `seqs` 换回主线序号：分叉点以内的照原样，抄进去的照对照表，别的（照说不会）不要。
    fn main_seqs(&self, seqs: &[Seq]) -> Vec<Seq> {
        seqs.iter()
            .filter_map(|seq| match seq.get() <= self.upto {
                true => Some(*seq),
                false => self.copies.get(&seq.get()).and_then(|msg| Seq::new(*msg)),
            })
            .collect()
    }
}

/// 几条（主线序号）进了回合编号是 `turn` 的那一轮：没有回合编号的（照说不会）什么都不做。
fn took(turn: Option<u64>, triggers: &[Seq]) -> Seen {
    match turn {
        Some(turn) => Seen::Took {
            turn,
            msgs: triggers.iter().map(|seq| seq.get()).collect(),
        },
        None => Seen::Nothing,
    }
}

/// 一条人话 `event` 写成抄进支线用的参数（「群里怎么叫她」第 7 条「抄」）：字块接起来；`as` 照原来那个人；`venue` 照原样、加上
/// `ambient`；图、文件块照原样写进 `attachments`。不是外部身份说的、没有命令编号、没有场所的格的是空的。
fn kept(event: &Event) -> Option<Kept> {
    let (Body::MessageUser(user), By::External(external)) = (&event.body, &event.by) else {
        return None;
    };
    let cause = event.cause.as_ref()?.as_str().to_string();
    let mut venue = serde_json::to_value(user.venue.as_ref()?).ok()?;
    venue["ambient"] = json!(true);
    let text: String = user
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect();
    let attachments: Vec<Value> = user
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Image(image) => {
                let name = image.name.as_ref()?;
                Some(json!({"blob": image.blob, "name": name, "media_type": image.media_type}))
            }
            Block::File(file) => {
                Some(json!({"blob": file.blob, "name": file.name, "media_type": file.media_type}))
            }
            _ => None,
        })
        .collect();
    let mut acting = json!({"external": external.id});
    if let Some(role) = external.role {
        acting["role"] = json!(role);
    }
    let mut params = json!({"text": text, "as": acting, "venue": venue});
    if !attachments.is_empty() {
        params["attachments"] = json!(attachments);
    }
    Some(Kept { cause, params })
}
