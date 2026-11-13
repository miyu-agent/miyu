//! 起标题（施工 3-8 五补，`docs/blueprint/kernel/session.md`「起标题」）：没起名的会话一轮答完，内核自己照这一刻落了盘的
//! 有效历史组装一次单独的辅助请求（`Assembler::title`，`kernel/request.md`「起标题的请求」），说完了记 `model.called`
//! （`purpose` 是 `title`）和 `session.meta_changed`（`by` 是内核）。照 Claude Code：人手动起名以外，没起名的它自己起
//! （2026-10-01 项目主人定）。
//!
//! - 什么时候起：一轮的 `turn.ended` 落了盘、这一轮有她带正文的回答、会话的标题从没动过（人改过、去掉过、内核起过的都算
//!   动过）、不是子会话（派它时的标题就是它的名字）、不是一次性的会话（2026-10-01 项目主人定）、试过的次数还没到上限、
//!   没有一次在路上。
//! - 试过几次从日志数：`purpose` 是 `title` 的 `model.called` 有几条，载入时照样数回来。在路上的那一次跟着重启丢了的
//!   不算，下一轮答完再试。
//! - 不属于哪一轮：两条事件都不带回合编号，没有 `cause`（没有谁要它），撤哪一轮都不跟着拿走，不进她的上下文。
//! - 说完了：正文取第一行、去掉前后空白，超过上限的截掉（不加省略号）。出错、没有正文的只记那一条 `model.called`；在路上
//!   时人改了名的，也只记那一条，不盖掉人起的。
//! - 人要求重起（`session.retitle`，施工 TT-1）：走同一条路、同一份指令，喂的换成最近几轮（`Assembler::retitle`），不看「标题动过」「说了不自动起」「一次性的会话」
//!   「试过几次」；起好了盖掉现在的标题，人起的也盖。有一次在路上的并进去，不再发；这时还组装不出来、回合正在进行的，等
//!   这一轮的 `turn.ended` 落了盘再起；空闲着也组装不出来的拒绝，`nothing_to_title`。
//! - 人要的标题算人起的（施工 TT-2，2026-11-13 core1 定）：`session.meta_changed` 的 `by` 是要它的那个命令的 `by`，同
//!   `set_meta`（会话列表照它算 `named`）；`model.called` 照旧是内核的。不带 `cause`：命令收下就回了，载入时照 `cause`
//!   认回命令编号会附上这一条，活着时却没有，两边对不上。
//! - 人要的没起成（施工 TT-3，2026-11-13 项目主人定）：超时、出错、取不出标题，标题不动，推一条 `session.retitle_failed`
//!   给订阅着的头（`by` 是要它的人）。自动起的没起成照旧不吵人。辅助请求最多等多久归会话那一层（`miyu-session` 的
//!   `port::ASIDE_LIMIT`），到时照出错送进来，这里不分。

use super::Session;
use super::action::{Action, Reason, accepted, rejected};
use super::aside::{Aside, Finished};
use crate::block::Block;
use crate::event::{
    Body, CallError, Cost, Event, MetaChanged, Purpose, RetitleFailed, Said, Transient,
    TransientBody, Usage,
};
use crate::id::{CommandId, Seq, TurnId};
use crate::origin::By;
use crate::time::Timestamp;

/// 人要求的重起标题没起成时给人看的那一句（施工 TT-3）：字在 `core/human/<语言>.json` 的 `said` 下。
const RETITLE_FAILED: &str = "core/sessions/retitle-failed";

/// 起标题的账：从日志一条条算，活着时每追加一条记一次，载入时照日志再走一遍，算出来的一样。
#[derive(Debug, Default)]
pub(super) struct Naming {
    /// 子会话（`session.created` 带 `parent`）：不起。
    child: bool,
    /// 造会话时说了不自动起（`session.created` 的 `titles` 是 `false`，施工 O-43）：不起。
    quiet: bool,
    /// 标题动过：有过带 `title` 的 `session.meta_changed`，人改的、去掉的、内核起的都算。
    named: bool,
    /// 试过几次：`purpose` 是 `title` 的 `model.called` 有几条。
    tries: u32,
    /// 最近一条有正文的回复在哪一轮。
    answered: Option<TurnId>,
    /// 最近设下的长期目标的原文（施工 X-8 补）：人还没说过话、只有续轮的会话照它起标题。
    goal: Option<String>,
    /// 有过不是模块发的话（人、场所里的人、别的会话、别的 harness）。
    spoke: bool,
    /// 人要求重起、还没起成（施工 TT-1），记着要它的是谁（施工 TT-2：起好的标题记成他起的）。只在内存里，载入以后没有。
    /// 标题一动（起好了、人又改了）就放下；没起成的也放下。
    asked: Option<By>,
}

impl Naming {
    /// 记下追加的一条。
    pub(super) fn note(&mut self, event: &Event) {
        match &event.body {
            Body::SessionCreated(created) => {
                self.child = created.parent.is_some();
                self.quiet = !created.titles;
            }
            Body::MetaChanged(changed) if changed.title.is_some() => {
                self.named = true;
                self.asked = None;
            }
            Body::ModelCalled(called) if called.purpose == Some(Purpose::Title) => self.tries += 1,
            Body::MessageAssistant(reply) if said(&reply.blocks) => self.answered = event.turn,
            Body::GoalSet(set) => self.goal = Some(set.text.clone()),
            Body::MessageUser(_) if !matches!(event.by, By::Module(_)) => self.spoke = true,
            _ => {}
        }
    }

    /// 还要起：不是子会话，没说不起，标题没动过，试过的不到 `tries` 次。
    fn wanted(&self, tries: u32) -> bool {
        !self.child && !self.quiet && !self.named && self.tries < tries
    }

    /// 只有续轮的会话的目标原文（施工 X-8 补）：设过目标、还没有谁说过话。
    fn goal_only(&self) -> Option<&str> {
        self.goal.as_deref().filter(|_| !self.spoke)
    }
}

/// 这几块里有正文：去掉空白不是空的。
fn said(blocks: &[Block]) -> bool {
    blocks.iter().any(|block| match block {
        Block::Text(text) => !text.text.trim().is_empty(),
        _ => false,
    })
}

impl Session {
    /// 该起标题了就起（「落了盘」里，`turn.ended` 落了盘的回合是 `closed`）：照这一刻落了盘的有效历史组装，名字是照到的
    /// 那一条。组装不出来的（快照里没有起标题的字、有效历史里没有带正文的回答）不起，也不算试过。
    ///
    /// 只有续轮的会话（施工 X-8 补：人在空会话里直接 `/goal`，她自己设了目标以后人还没说话的不算，那时已经有人的话）：不请求
    /// 模型，拿目标原文当标题，`session.meta_changed`（`by` 内核）当场追加。有效历史里只有续轮消息，模型会照它的标签起名。
    ///
    /// 人要求重起、等着这一轮完的（施工 TT-1）：随便哪一轮的 `turn.ended` 落了盘就起，别的几条不看。
    pub(super) fn title_up(&mut self, at: Timestamp, closed: &[TurnId]) -> Option<Action> {
        let titles = self.policy.titles?;
        let answered = self.naming.answered?;
        let due = match self.naming.asked {
            Some(_) => !closed.is_empty(),
            None => !self.oneshot && self.naming.wanted(titles.tries) && closed.contains(&answered),
        };
        if self.titling.is_some() || self.restarting || !due {
            return None;
        }
        self.title_now(at, titles)
    }

    /// 人要求重起标题（`session.retitle`，施工 TT-1）：收下就回，不等模型；标题经 `session.meta_changed` 到，`by` 是这里的
    /// `by`（施工 TT-2）。
    ///
    /// 有一次在路上的并进去；没有的当场照 [`Session::title_now`] 起；这时组装不出来、回合正在进行的等这一轮完
    /// （[`Session::title_up`]）；空闲着也组装不出来、策略里没有起标题的数的，拒绝，`nothing_to_title`。
    pub(super) fn retitle(&mut self, id: CommandId, by: By, at: Timestamp) -> Vec<Action> {
        let Some(titles) = self.policy.titles else {
            return vec![rejected(id, Reason::NothingToTitle)];
        };
        self.naming.asked = Some(by);
        let started = match self.titling.is_some() {
            true => None,
            false => self.title_now(at, titles),
        };
        if started.is_none() && self.titling.is_none() && self.turn.is_none() {
            self.naming.asked = None;
            return vec![rejected(id, Reason::NothingToTitle)];
        }
        self.recent.insert(id.clone(), Vec::new());
        let mut actions = vec![accepted(id, Vec::new())];
        actions.extend(started);
        actions
    }

    /// 照这一刻落了盘的有效历史起一次：只有续轮的拿目标原文当场记下，别的组装请求发出去（人要求重起的喂最近几轮，自动起的
    /// 只喂第一轮）；组装不出来的没有。人要的那一条记成他起的（施工 TT-2）。
    fn title_now(&mut self, at: Timestamp, titles: super::Titles) -> Option<Action> {
        if let Some(goal) = self.naming.goal_only().map(str::to_string) {
            let by = self.naming.asked.clone();
            let event = self.titled(at, by, shorten(goal.trim(), titles.chars));
            return Some(Action::Append(vec![event]));
        }
        let history = self.history.until(self.stored?);
        // 人要求重起的喂最近几轮，自动起的只喂第一轮（施工 TT-1）：指令、上限是同一份。
        let (request, upto) = match self.naming.asked {
            Some(_) => self.policy.assembler.retitle(&history)?,
            None => self.policy.assembler.title(&history)?,
        };
        self.titling = Some(Aside::new(upto, request.messages.len()));
        Some(Action::Aside {
            purpose: Purpose::Title,
            upto,
            request,
        })
    }

    /// 起标题的请求说完了：记 `model.called`（`by` 是内核）；写成了、标题这时还没人动过的（人要求重起的不看动没动过），接着记
    /// `session.meta_changed`（`by` 是内核，人要的是要它的人，施工 TT-2），同一批。两条都不带回合编号、没有 `cause`。
    pub(super) fn title_ended(
        &mut self,
        at: Timestamp,
        upto: Seq,
        spent: (Option<Usage>, Option<Cost>),
        error: Option<CallError>,
    ) -> Vec<Action> {
        let Some(aside) = self.titling.take_if(|aside| aside.upto == upto) else {
            return Vec::new();
        };
        let next = self.ledger.next_seq();
        let Finished { called, text } = aside.finish(
            at,
            Purpose::Title,
            spent,
            error,
            next,
            "the title reply has no text",
        );
        let mut events = vec![self.record_aside(at, None, Body::ModelCalled(called))];
        let chars = self.policy.titles.map_or(0, |titles| titles.chars);
        // 人要求重起的那一次盖得掉动过的标题（施工 TT-1）；没起成的也放下，不留到下一轮。
        let asked = std::mem::take(&mut self.naming.asked);
        let failed = match (text, asked) {
            (Ok(text), asked) if asked.is_some() || !self.naming.named => {
                events.push(self.titled(at, asked, shorten(&text, chars)));
                None
            }
            // 人要的没起成（施工 TT-3）：告诉头，标题不动。自动起的没起成不吵人。
            (Err(error), Some(by)) => Some(retitle_failed(at, by, error)),
            _ => None,
        };
        let mut actions = vec![Action::Append(events)];
        actions.extend(failed.map(Action::PushTransient));
        actions
    }

    /// 记下起好的标题：一条只写 `title` 的 `session.meta_changed`，不带回合编号、没有 `cause`。`asked` 是要它的人（施工
    /// TT-2：人要的算他起的，同 `set_meta`），没人要的 `by` 是内核。
    fn titled(&mut self, at: Timestamp, asked: Option<By>, title: String) -> Event {
        let changed = MetaChanged {
            title: Some(title),
            pinned: None,
            project: None,
        };
        self.meta.note(&changed);
        self.record_outside(
            at,
            asked.unwrap_or(By::Kernel),
            None,
            Body::MetaChanged(changed),
        )
    }
}

/// 人要求的重起标题没起成（施工 TT-3）：一条推给头的 `session.retitle_failed`，`by` 是要它的人，不属于哪一轮、没有 `cause`
/// （命令收下就回过了）。给人看的那一句是 [`RETITLE_FAILED`]。
fn retitle_failed(at: Timestamp, by: By, error: CallError) -> Transient {
    Transient {
        at,
        turn: None,
        by,
        cause: None,
        body: TransientBody::RetitleFailed(RetitleFailed {
            error,
            human: Said::new(RETITLE_FAILED),
        }),
    }
}

/// 回复里的标题：第一行，去掉前后空白，超过 `chars` 个字（Unicode 字符）的截到 `chars` 个、再去掉末尾的空白，不加省略号。
/// `text` 已经去掉了前后空白、不是空的，第一行也就不是空的。
fn shorten(text: &str, chars: usize) -> String {
    let line = text.lines().next().unwrap_or_default().trim();
    let cut: String = line.chars().take(chars).collect();
    cut.trim_end().to_string()
}
