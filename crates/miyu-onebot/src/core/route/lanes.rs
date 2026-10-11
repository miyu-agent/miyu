//! 分派和支线（施工 O-36，`onebot.md` 第一条「群里怎么叫她」第 1、6、7、9 条；`chat.md` 第四条；`session/fork.md`）：判下来要回
//! 的照群聊内核的 `dispatch` 做，支线推来的事件收进主线的投影。纯逻辑的那一半在 `projection/lanes.rs`。
//!
//! - `StartMain`：`session.respond` 主线，给每条支线在回的每一条带一块「另一条线正在回」（模板是出厂数据，`rules/facts.rs`）。
//! - `JoinMain`、`Queue`：`session.respond` 主线，不带事实（施工单「要拍板的」第 3 条：核心没有「排到下一轮」，并进来就是）。
//! - `JoinLane`：判的几条里比分叉点晚、还没抄进去的先抄（`session.send`，命令编号加 `/branch`，`venue` 带 `ambient`），再
//!   `session.respond` 支线。
//! - `Fork`：`session.fork {seal_after: 1}`、订阅子会话（`after` 是分叉点）、把留着的人话里比分叉点晚的照先后抄进去，最后
//!   `session.respond` 支线，给主线在回的每一条带一块。分叉被拒的照 `Queue` 办；订阅不上、开不了轮的支线当场去掉（不然它不会
//!   封存，一直占着位置）。
//! - 支线刚好封存了（`session_sealed`）：先把留着的推送收进来（封存的那一条在里面，支线跟着去掉），重新分派一次，只重来一次。
//! - 支线推来的（[`Route::lane_heard`]）：换回主线序号收进主线的投影，贴着的表情照 `(line, turn)` 记、摘，「不说话」照子会话
//!   编号认，她的话照主线的出站链发；`session.sealed` 退订。

use miyu_chat::{Dispatch, Venue, dispatch};
use miyu_kernel::event::Event;
use serde_json::{Value, json};

use super::judges::Tag;
use super::projection::{Carry, Seen};
use super::{Route, applied, reply_text};
use crate::TARGET;
use crate::core::{Gone, reason};

/// 「另一条线正在回」那一块事实的 `kind`（「群里怎么叫她」第 7 条「事实」）。
const ANOTHER_LINE: &str = "another-line";

/// 拒绝码：支线封存了（`session/fork.md`「封存」）。
const SEALED: &str = "session_sealed";

/// 开一轮、并进一轮怎么样了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Responded {
    /// 开了一轮、并进了一轮。
    Asked,
    /// 被拒了（记了运行日志）。
    Refused,
    /// 那条支线已经封存了：重新分派。
    Sealed,
}

impl Route {
    /// 群会话 `session` 里判下来要回的几条 `msgs`（最后一条的命令编号、平台编号、场所是 `tag`）照分派做（第 6、7 条）：交回开没
    /// 开成（成了的贴表情）。支线刚好封存了的收进留着的推送、重新分派一次。
    ///
    /// # Errors
    ///
    /// 写不出去、等的时候核心断开。
    pub(super) async fn dispatched(
        &mut self,
        session: &str,
        msgs: &[u64],
        tag: &Tag,
    ) -> Result<bool, Gone> {
        for retried in [false, true] {
            match self.dispatch_once(session, msgs, tag).await? {
                Responded::Asked => return Ok(true),
                Responded::Refused => return Ok(false),
                Responded::Sealed if !retried => self.catch_up(session).await?,
                Responded::Sealed => {
                    tracing::warn!(target: TARGET, venue = %tag.venue, message = tag.number, reason = SEALED, "respond refused");
                }
            }
        }
        Ok(false)
    }

    /// 分派一次：照投影的 `Lines`、这个群的 `parallel` 问群聊内核，照结论做。发的人认不出的（照说不会）照并进主线办。
    async fn dispatch_once(
        &mut self,
        session: &str,
        msgs: &[u64],
        tag: &Tag,
    ) -> Result<Responded, Gone> {
        let parallel = match self
            .peers
            .get(session)
            .and_then(|peer| Venue::parse(&peer.venue))
        {
            Some(venue) => applied::parallel(&self.applied(&venue)),
            None => 0,
        };
        let Some(group) = self.groups.get(session) else {
            return self.respond(session, msgs, &[], tag).await;
        };
        let lines = group.lines(parallel);
        let decided = match msgs.last().and_then(|seq| group.sender(*seq)) {
            Some(sender) => dispatch(sender, &lines),
            None => Dispatch::JoinMain,
        };
        match decided {
            Dispatch::StartMain => {
                let aim = msgs
                    .last()
                    .and_then(|seq| group.msg(*seq))
                    .map(str::to_string);
                let facts = self.another_line(group.answering(true), aim.as_deref());
                self.respond(session, msgs, &facts, tag).await
            }
            Dispatch::JoinMain | Dispatch::Queue => self.respond(session, msgs, &[], tag).await,
            Dispatch::JoinLane(index) => match group.lane_at(index).map(str::to_string) {
                Some(lane) => self.join_lane(session, &lane, msgs, tag).await,
                None => self.respond(session, msgs, &[], tag).await,
            },
            Dispatch::Fork => self.fork(session, msgs, tag).await,
        }
    }

    /// 并进群会话 `session` 的支线 `lane`：判的几条 `msgs` 里还没抄进去的先抄，再 `session.respond` 支线，不带事实。
    async fn join_lane(
        &mut self,
        session: &str,
        lane: &str,
        msgs: &[u64],
        tag: &Tag,
    ) -> Result<Responded, Gone> {
        let carries = self
            .groups
            .get(session)
            .map(|group| group.to_copy(lane, Some(msgs)))
            .unwrap_or_default();
        for carry in carries {
            if self.carry(session, lane, carry).await? == Responded::Sealed {
                return Ok(Responded::Sealed);
            }
        }
        self.respond_lane(session, lane, msgs, &[], tag).await
    }

    /// 给群会话 `session` 分叉一条支线回 `msgs`（第 7 条 `Fork`）。
    async fn fork(&mut self, session: &str, msgs: &[u64], tag: &Tag) -> Result<Responded, Gone> {
        let id = format!("{}/fork", tag.id);
        let params = json!({"session": session, "seal_after": 1});
        let reply = self.core.call_as(&id, "session.fork", params).await?;
        let result = &reply["result"];
        let (Some(lane), Some(upto)) = (result["session"].as_str(), result["upto"].as_u64()) else {
            let reason = reason(&reply).unwrap_or("bad_reply");
            tracing::warn!(target: TARGET, venue = %tag.venue, message = tag.number, reason, "lane not forked");
            return self.respond(session, msgs, &[], tag).await;
        };
        let lane = lane.to_string();
        let Some(group) = self.groups.get_mut(session) else {
            return Ok(Responded::Refused);
        };
        // 主线这一轮在回的，照分叉这一刻定死（18 第八节「要守的三条」第 2 条）。
        let answering = group.answering(false);
        let aim = msgs
            .last()
            .and_then(|seq| group.msg(*seq))
            .map(str::to_string);
        group.forked(&lane, upto, msgs);
        tracing::info!(target: TARGET, venue = %tag.venue, message = tag.number, lane, upto, "lane forked");
        let facts = self.another_line(answering, aim.as_deref());
        let responded = match self.follow_lane(&lane, upto).await? {
            true => {
                self.catch_up(session).await?;
                let carries = self
                    .groups
                    .get(session)
                    .map(|group| group.to_copy(&lane, None))
                    .unwrap_or_default();
                for carry in carries {
                    self.carry(session, &lane, carry).await?;
                }
                self.respond_lane(session, &lane, msgs, &facts, tag).await?
            }
            false => Responded::Refused,
        };
        if responded != Responded::Asked {
            // 开不了轮的支线不会封存：去掉、退订，不让它一直占着位置。
            if let Some(group) = self.groups.get_mut(session) {
                group.dropped(&lane);
            }
            self.unfollow(&lane).await?;
        }
        Ok(responded)
    }

    /// 把一条人话 `carry` 抄进群会话 `session` 的支线 `lane`（第 7 条「抄」），记进序号对照表。核心拒了的记一行：封存了的交回
    /// `Sealed`，别的照样往下走（`respond` 里就没有这一条）。
    async fn carry(&mut self, session: &str, lane: &str, carry: Carry) -> Result<Responded, Gone> {
        let mut params = carry.params;
        params["session"] = json!(lane);
        let reply = self.core.call_as(&carry.id, "session.send", params).await?;
        match (reason(&reply), reply["result"]["events"][0].as_u64()) {
            (None, Some(at)) => {
                if let Some(group) = self.groups.get_mut(session) {
                    group.copied(lane, at, carry.msg);
                }
                Ok(Responded::Asked)
            }
            (Some(SEALED), _) => Ok(Responded::Sealed),
            (reason, _) => {
                let reason = reason.unwrap_or("bad_reply");
                tracing::warn!(target: TARGET, session, lane, reason, "copy refused");
                Ok(Responded::Refused)
            }
        }
    }

    /// 支线 `lane` 照判的几条 `msgs`（主线序号）开一轮、并进一轮：换成支线里的序号，带上事实 `facts`。一条都换不出来的（照说
    /// 不会：都抄过了）记一行，不开。
    async fn respond_lane(
        &mut self,
        session: &str,
        lane: &str,
        msgs: &[u64],
        facts: &[Value],
        tag: &Tag,
    ) -> Result<Responded, Gone> {
        let to = self
            .groups
            .get(session)
            .map(|group| group.targets(lane, msgs))
            .unwrap_or_default();
        if to.is_empty() {
            tracing::warn!(target: TARGET, venue = %tag.venue, message = tag.number, lane, "nothing to respond on the lane");
            return Ok(Responded::Refused);
        }
        self.respond(lane, &to, facts, tag).await
    }

    /// 照记下的几条 `to` 在会话 `target`（主线或者支线）开一轮（第 7 条）：`session.respond {session, to, facts?}`，命令编号照
    /// 最后一条的 `tag` 加 `/respond`；正在跑一轮的核心并进去。核心说已经当过触发、不是旁听的不再开，记一行；封存了的交回
    /// `Sealed`，调的一方重新分派。
    async fn respond(
        &mut self,
        target: &str,
        to: &[u64],
        facts: &[Value],
        tag: &Tag,
    ) -> Result<Responded, Gone> {
        let id = format!("{}/respond", tag.id);
        let mut params = json!({"session": target, "to": to});
        if !facts.is_empty() {
            params["facts"] = json!(facts);
        }
        let reply = self.core.call_as(&id, "session.respond", params).await?;
        let (venue, number) = (&tag.venue, tag.number);
        match reason(&reply) {
            None => {
                tracing::info!(target: TARGET, venue = %venue, message = number, session = target, "respond asked");
                Ok(Responded::Asked)
            }
            Some(SEALED) => Ok(Responded::Sealed),
            Some(reason @ ("already_answered" | "not_ambient")) => {
                let messages = &reply["error"]["data"]["messages"];
                tracing::info!(target: TARGET, venue = %venue, message = number, reason, messages = %messages, "respond refused");
                Ok(Responded::Refused)
            }
            Some(reason) => {
                tracing::warn!(target: TARGET, venue = %venue, message = number, reason, "respond refused");
                Ok(Responded::Refused)
            }
        }
    }

    /// 别的线在回的、平台编号是 `msgs` 的那几条各写一块「另一条线正在回」，末尾点明这条线回的是平台编号 `aim` 的那一条（第 7 条
    /// 「事实」）。这条线回的那一条没有平台编号的（照说不会）不带；写不出的（照说不会：起来时查过字段）记一行、不带。
    fn another_line(&self, msgs: Vec<String>, aim: Option<&str>) -> Vec<Value> {
        let Some(aim) = aim else {
            return Vec::new();
        };
        msgs.iter()
            .filter_map(|msg| {
                let fields = [("msg", msg.as_str()), ("aim", aim)].into_iter().collect();
                match self.rules.another_line().render(&fields) {
                    Ok(text) => Some(json!({"kind": ANOTHER_LINE, "text": text})),
                    Err(error) => {
                        tracing::warn!(target: TARGET, error = %error, "another line not written");
                        None
                    }
                }
            })
            .collect()
    }

    /// 订阅支线 `lane`，补 `after` 以后的（第 1 条：分叉点、掉了队收到的最后一条）。订阅不上的记一行、交回假。
    async fn follow_lane(&mut self, lane: &str, after: u64) -> Result<bool, Gone> {
        let params = json!({"session": lane, "stream": "events", "after": after});
        let reply = self.core.call("subscribe", params).await?;
        if let Some(reason) = reason(&reply) {
            tracing::warn!(target: TARGET, session = lane, reason, "not subscribed");
            return Ok(false);
        }
        Ok(true)
    }

    /// 退订支线 `lane`（封存了、用不成了）。核心拒了的（已经不在了）记一行调试日志。
    async fn unfollow(&mut self, lane: &str) -> Result<(), Gone> {
        let params = json!({"session": lane, "stream": "events"});
        let reply = self.core.call("unsubscribe", params).await?;
        if let Some(reason) = reason(&reply) {
            tracing::debug!(target: TARGET, session = lane, reason, "not unsubscribed");
        }
        Ok(())
    }

    /// 支线 `lane` 是哪个群会话（主线）的；不是谁的支线的是空的。
    pub(super) fn main_of_lane(&self, lane: &str) -> Option<String> {
        self.groups
            .iter()
            .find(|(_, group)| group.has_lane(lane))
            .map(|(session, _)| session.clone())
    }

    /// 会话 `session` 的主线：是支线的交回它的群会话，别的交回它自己（施工 O-36：平台工具照主线找投影、发到哪）。
    pub(super) fn main_of(&self, session: &str) -> String {
        self.main_of_lane(session)
            .unwrap_or_else(|| session.to_string())
    }

    /// 群会话 `session` 的支线 `lane` 推来的一条 `pushed`（`event`；`resync` 照收到的最后一条再订阅）：收进主线的投影，照交回的做
    /// （见模块开头）。
    ///
    /// # Errors
    ///
    /// 她的话入队时、退订时写不出去、等的时候核心断开。
    pub(super) async fn lane_heard(
        &mut self,
        session: &str,
        lane: &str,
        pushed: &Value,
    ) -> Result<(), Gone> {
        if pushed["method"] == "resync" {
            let last = self
                .groups
                .get(session)
                .and_then(|group| group.lane_last(lane));
            if let Some(last) = last {
                self.follow_lane(lane, last).await?;
            }
            return Ok(());
        }
        let raw = &pushed["params"]["event"];
        if raw["seq"].as_u64().is_none() {
            return Ok(());
        }
        let event: Event = match serde_json::from_value(raw.clone()) {
            Ok(event) => event,
            Err(error) => {
                tracing::warn!(target: TARGET, session = lane, error = %error, "event not understood");
                return Ok(());
            }
        };
        let quiet = self.quiet.heard(lane, raw);
        let Some(group) = self.groups.get_mut(session) else {
            return Ok(());
        };
        match group.lane_take(lane, &event) {
            Seen::Took { turn, msgs } => self.reactions.taken(session, lane, &msgs, turn),
            Seen::Ended(turn) => self.reactions.done(session, lane, turn),
            Seen::Spoke(_) if quiet => self.hushed(session, &reply_text(raw)),
            Seen::Spoke(speaking) => self.speak(session, speaking, &reply_text(raw)).await?,
            Seen::Sealed => {
                tracing::info!(target: TARGET, session, lane, "lane sealed");
                // 这条支线取了几次（`fetch_media`）不用再记。
                self.fetches.remove(lane);
                self.unfollow(lane).await?;
            }
            Seen::Nothing => {}
        }
        Ok(())
    }
}
