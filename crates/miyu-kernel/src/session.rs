//! 会话：内核的状态机（`internal/designs/02-内核.md` 第四节「输入、动作、命令怎么写」、
//! 第六节「回合怎么开、请求怎么发」）。
//!
//! 送进一条输入，出来一串动作。会话不做 I/O：要追加的事件、要回应的命令、要推送的事件、
//! 要跑的挂接点、要发的请求，都写成动作交给执行器；事件同步到磁盘以后，执行器送一条
//! 「落盘了」进来，会话这才推送、回应，回合这才往下走（`07-存储.md` S4）。
//!
//! 每收到一次命令，恰好回应一次（不变量 7）；同一个编号只生效一次（不变量 9）。

mod action;
mod approval;
mod aside;
mod asked;
mod blank;
mod breaker;
mod call;
mod chain;
mod clear;
mod compaction;
mod configure;
mod dispatch;
mod dream;
mod driver_ask;
mod fork;
mod goal;
mod grants;
mod input;
mod interrupt;
mod jobs;
mod limits;
mod load;
mod manual;
mod messages;
mod meta;
mod nested;
mod note;
mod overflow;
mod peers;
mod permission;
mod policy;
mod prepare;
mod question;
mod queue;
mod rebuild;
mod recap;
mod recast;
mod recent;
mod record;
mod redo;
mod replies;
mod report;
mod respond;
mod restart;
mod retry;
mod revert;
mod seal;
mod send;
mod shell;
mod shelve;
mod shorten;
mod sight;
mod spans;
mod states;
mod status;
mod step;
mod summary;
mod supersede;
mod title;
mod todos;
mod tools;
mod truncated;
mod turn;
mod voice;
mod watch;
mod workspace;

pub use action::{Action, Outcome, Reason, StopReason};
pub use configure::Replaced;
pub use driver_ask::{AutoDeny, DriverAnswer, DriverAsked};
pub use fork::{BaseFeed, ForkFeed, ForkPlan};
pub use input::{
    Answer, Appended, Command, ExtEvent, GoalChange, Injection, Input, Limits, Queued, Received,
    Reread, Verdict,
};
pub use limits::{Breakdown, ContextLimits, Frame, Said, breakdown_in};
pub use load::{LoadError, Loader};
pub use messages::Subagent;
pub use nested::{NestedAction, NestedAnswer, NestedInput, NestedRefusal, NestedRunning};
pub use policy::{
    Compaction, NestedLimits, Notes, Pause, Peers, Policy, Rebuild, Reports, Shorten, SkillRebuild,
    Titles,
};
pub use report::Upward;
pub use watch::{CHATTY, LINE_CHARS};

use crate::apart;
use crate::event::{Body, Event, Permission, SessionCreated, ToolResult, ToolStatus};
use crate::facts::Environment;
use crate::history::History;
use crate::id::{CommandId, Seq, SessionId, TurnId};
use crate::ledger::Ledger;
use crate::origin::By;
use crate::request::Fingerprint;
use crate::time::Timestamp;
use action::{accepted, rejected, rejected_about};
use jobs::Arrived;
use recent::Recent;
use revert::ReadingBack;
use turn::Turn;

/// 一个会话的状态机。
#[derive(Debug)]
pub struct Session {
    /// 这个会话的编号：执行器造会话、载入时交进来，日志所在的目录就叫它。事实 `session` 写它（施工 1-13 再补）。
    id: SessionId,
    /// 日志的账本：给新事件编序号，追加之前照规矩查一遍。
    ledger: Ledger,
    /// 有效历史，组装请求用。事件追加时就交给它。
    history: History,
    /// 追加了、还没落盘的事件，照先后。
    unstored: Vec<Event>,
    /// 落了盘的最后一条；还没有落过盘就是没有。
    stored: Option<Seq>,
    /// 等事件落了盘才回应的命令：编号、要等的事件的序号、回应的结局，照收到的先后（施工 3-8 四补：回顾回的不是「接受了」）。
    waiting: Vec<(CommandId, Vec<Seq>, Outcome)>,
    /// 最近接受的命令编号。
    recent: Recent,
    /// 冻结在会话上的策略，和执行器先放着、回合开始时换上的新的一份（施工 P-1 再补，`policy.rs`）。
    policy: policy::Held,
    /// 会话所在的环境：时区、工作目录。
    environment: Environment,
    /// 现在的权限：人最近一次切成的。
    permission: Permission,
    /// 实际生效的权限：收紧的当场换，放宽的等下一次请求（`02-内核.md` 第六节「权限级别怎么切」）。
    effective: Permission,
    /// 正在进行的回合；空闲时没有。
    turn: Option<Turn>,
    /// 这个会话上一次请求的指纹，比出下一次的第一处不同。只在内存里。
    last_request: Option<Fingerprint>,
    /// 上一次主请求里人这一边的外壳，下一次接着算（施工 FT-1，`forged.rs`）。只在内存里。
    facing: crate::forged::Facing,
    /// 结束了、`turn.ended` 还没落盘的回合，和那一条的序号：落了盘才跑回合结束的挂接点。
    closing: Vec<(TurnId, Seq)>,
    /// 撤掉压缩的撤销正在读回日志（施工 6-9）：交出去了，读回的还没来。
    reading: Option<ReadingBack>,
    /// 模型的限额，执行器交来的；没交过的不主动压缩（施工 6-2 上）。只在内存里。
    limits: Option<Limits>,
    /// 一次性的会话：`miyu ask` 开的（`session.created` 的 `oneshot`）。没人看着的时候回报不叫醒她（施工 7-2）。
    oneshot: bool,
    /// 整理会话（`session.created` 的 `dream`，施工 D-1 上）：主请求的 `model.called` 用途记 `memory`。
    dream: bool,
    /// 有头订阅着（施工 7-2，[`Input::Watched`]）：只在内存里，造会话、载入以后当没人看着。
    watched: bool,
    /// 闲着时到的、会叫醒她、这时却开不了一轮的回报，照先后（施工 7-2，`jobs.rs`）：随便哪一轮开了就清掉；恢复了撤销、这时开得了，由最后那条接着开。
    deferred: Vec<Arrived>,
    /// 太吵、已经发过停的动作的监视命令（施工 MN-1 上，`watch.rs`）：再来的批次不记、不再发。只在内存里：重启了命令也就停了。
    chatty: std::collections::BTreeSet<crate::id::JobId>,
    /// 收到了「要重启了」（施工 7-3）：要关了，之后到的回报只记下、不开轮。只在内存里。
    restarting: bool,
    /// 子会话欠着父会话的回报（施工 7-6，`report.rs`）：每追加一条记一次。
    duty: report::Duty,
    /// 现在的标题、置顶（施工 3-8 三补，`meta.rs`）：日志里的 `session.meta_changed` 一路算的。
    meta: meta::Meta,
    /// 在路上的那一次回顾（施工 3-8 四补，`recap.rs`）：只在内存里，载入以后没有。
    recapping: Option<recap::Recapping>,
    /// 起标题的账（施工 3-8 五补，`title.rs`）：每追加一条记一次。
    naming: title::Naming,
    /// 在路上的那一次起标题（施工 3-8 五补）：只在内存里，载入以后没有。
    titling: Option<aside::Aside>,
    /// 提前压好的账（施工 6-11 上，`prepare.rs`）：只在内存里，载入以后没有。
    prepare: prepare::Prepare,
    /// 会话的引用和最近一次换模型写在第几条（施工 8-10，`configure.rs`）：每追加一条记一次。
    reference: configure::Reference,
    /// 转述过哪些图、哪些正在转（施工 8-17，`sight.rs`）：转述过的每追加一条记一次。
    sight: sight::Sight,
    /// 本会话放行过的规则（施工 D-1，`grants.rs`）：每追加一条记一次，交给链。
    grants: grants::Grants,
    /// 跑了几轮、跑满几轮封存（施工 C-8 下，`seal.rs`）：每追加一条记一次。
    rounds: seal::Rounds,
    /// 她干活的时间，等人不算（施工 E-M 下，`waits.rs`）：每追加一条记一次。
    busy: crate::waits::Busy,
    /// 被回报叫醒的这一串用了几步（审计 R3E-2，`chain.rs`）：每追加一条记一次。
    chain: chain::Chain,
    /// 还没跑的人打的命令（施工 SH-1 上，`shell.rs`）：每追加一条记一次。
    shells: shell::Shells,
    /// 上次告诉头的待办（施工 D-3，`todos.rs`）：变了才推 `todos.changed`。只在内存里。
    told_todos: Vec<crate::event::Todo>,
    /// 上次告诉头的「因为全部做完而清空的」那一份（施工 9-8 九补）：没写过就一次全做完的，清单前后都是空的，也要推。
    told_done: Vec<crate::event::Todo>,
    /// 上次告诉头的包的状态（施工 TW-1，`states.rs`）：变了才推 `package.state_changed`。只在内存里。
    told_states: states::Told,
    /// 嵌套调用（施工 M-1，`nested.rs`）：只在内存里，载入以后没有。
    nested: nested::Nested,
    /// 收成了槽（施工 LW-10 下，`shelve.rs`）时算好的「有没有能压的」：收着时答 [`Session::compactable`]。没收着的、收着又送了要看
    /// 历史的输入（读不回来的那段时间）的没有。
    rested: Option<bool>,
    /// 处理这一条输入时了结的驱动的确认要回的话（施工 ACP-8，`driver_ask.rs`）：[`Session::handle`] 收尾时一起交出去。
    answering: Vec<(Option<Seq>, Action)>,
}

impl Session {
    /// 造一个会话：追加第 1 条事件 `session.created`，它的 `cause` 是造会话的那个命令（`04-核心协议.md` 的 `session.create`）。这一条落了盘，再回应这个命令。
    ///
    /// 会话的编号 `session`、冻结在会话上的策略和会话所在的环境，由执行器一起交进来；开始时的权限取自 `created`。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：第 1 条是 `session.created`，账本收它。
    pub fn create(
        session: SessionId,
        id: CommandId,
        by: By,
        at: Timestamp,
        created: SessionCreated,
        policy: Policy,
        environment: Environment,
    ) -> (Session, Vec<Action>) {
        let ledger = Ledger::for_session(session.clone());
        let history = History::default().owned_by(session.clone());
        let mut session = Session::blank(session, ledger, history, &created, policy, environment);
        let event = session.record(
            at,
            by,
            Some(id.clone()),
            Body::SessionCreated(Box::new(created)),
        );
        session.accept(id, vec![event.seq]);
        (session, vec![Action::Append(vec![event])])
    }

    /// 空闲：没有在跑的回合，也没有结束了、`turn.ended` 还没落盘的，也没在读回日志。核心看它决定能不能空闲
    /// 退出（`12-进程形态与分发.md` 第二节，施工 3-9 上）。
    pub fn idle(&self) -> bool {
        self.turn.is_none() && self.closing.is_empty() && self.reading.is_none()
    }

    /// 会话现在在哪个目录里干活：头最近一次报来的，回合开始时这一轮照它（施工 8-4）。纯查询：会话 actor 在回合开始时照它
    /// 读一次项目配置（`internal/blueprint/config.md`「怎么走」第八条第 3 条）。
    pub fn cwd(&self) -> &str {
        &self.environment.cwd
    }

    /// 送进一条输入，出来一串动作。
    ///
    /// # Panics
    ///
    /// 内核自己造出的事件过不了账本时 panic：那是内核的 bug，不是命令的拒绝。
    ///
    /// 收成了槽的（[`Session::shelve`]）执行器先放回来再送要看历史的输入（[`Session::needs_history`]，`shelve.rs`）。
    pub fn handle(&mut self, input: Input) -> Vec<Action> {
        self.forget_rested(&input);
        let mut actions = self.react(input);
        self.splice_nested(&mut actions);
        actions.extend(self.dispatch_nested());
        actions.extend(self.answers_now());
        actions
    }

    /// 到第 `upto` 条为止落了盘：先推送这些事件，再回应事件全落了盘的命令（`04-核心协议.md`
    /// 第六节第 2 条：先见结果，后见回应），再跑结束了的回合的挂接点、向上回报、起标题（施工 3-8 五补），然后回合往下走。`upto` 超出追加过的，多出来的
    /// 不算；不比上一次往后的，什么都不做。`at` 是落完盘的时刻。
    fn stored(&mut self, at: Timestamp, upto: Seq) -> Vec<Action> {
        let split = self.unstored.partition_point(|event| event.seq <= upto);
        if split == 0 {
            return Vec::new();
        }
        let pushed: Vec<Event> = self.unstored.drain(..split).collect();
        self.stored = pushed.last().map(|event| event.seq);
        // 收着时追加的落了盘，并进槽里只记序号（施工 LW-36，`history/shelf.rs`）。
        self.history.landed(upto);
        let mut actions = vec![Action::Push(pushed)];
        self.answers_stored(upto);
        actions.extend(self.answer_nested(upto));
        actions.extend(self.todos_changed(at));
        actions.extend(self.states_changed(at));
        for (id, events, outcome) in std::mem::take(&mut self.waiting) {
            if self.is_stored(&events) {
                actions.push(Action::Reply { id, outcome });
            } else {
                self.waiting.push((id, events, outcome));
            }
        }
        let closed = self.closed();
        actions.extend(closed.iter().map(|&turn| Action::RunTurnEndHooks { turn }));
        actions.extend(self.report_up());
        actions.extend(self.title_up(at, &closed));
        actions.extend(self.advance(at));
        actions
    }
}

#[cfg(test)]
mod tests;
