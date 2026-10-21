//! 造替身（施工 7-5 从 `stage.rs` 挪出来）：人开的会话、`miyu ask` 开的一次性会话、别的会话派出来的子会话、分叉出来的会话
//! （施工 C-8 下）；载入「磁盘」。

use std::collections::VecDeque;
use std::num::NonZeroU32;

use super::stage::Stage;
use crate::event::{Event, Forked, SessionCreated};
use crate::facts::Environment;
use crate::id::SessionId;
use crate::origin::By;
use crate::session::{Action, LoadError, Policy, Session};
use crate::time::Timestamp;

use super::stage::command_id;

/// 替身造的会话的编号：人开的、一次性的都是它（施工 1-13 再补：事实 `session` 写它）。
pub const SESSION: &str = "01a0f233-cfec-7023-8ed5-2a037a1d5ec8";

/// 替身造的子会话自己的编号，和父会话的不一样：子会话的事实 `session` 写它（施工 1-13 再补）。
pub const CHILD_SESSION: &str = "01a0f234-0a1b-7c2d-8e3f-4a5b6c7d8e9f";

/// 造会话时的样子：alice 的会话，在本机，工作区的权限，只读关着。
const CREATED: &str = r#"{"owner":"alice","venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}"#;

/// 造的是哪一种会话。
enum Opening {
    /// 人开的会话。
    Main,
    /// `miyu ask` 开的一次性会话（施工 7-2）。
    Oneshot,
    /// 这个会话派出来的子会话（施工 7-5）。
    Child(SessionId),
    /// 从 [`SESSION`] 的日志分叉出来的（施工 C-8 下）：底子、跑完几轮封存。
    Fork(Vec<Event>, Option<NonZeroU32>),
}

impl Stage {
    /// 在 `now` 这一刻，照 `policy` 造一个会话，在 `environment` 里。造会话的命令编号是 `cmd-0`。
    ///
    /// # Panics
    ///
    /// 造会话那一条的写法坏了才会：那是替身自己的 bug。
    pub fn new(
        policy: impl Fn() -> Policy + 'static,
        environment: Environment,
        now: Timestamp,
    ) -> Stage {
        Stage::created(policy, environment, now, Opening::Main)
    }

    /// 同上，造的是一次性的会话：`miyu ask` 开的那种（施工 7-2）。
    ///
    /// # Panics
    ///
    /// 同上。
    pub fn oneshot(
        policy: impl Fn() -> Policy + 'static,
        environment: Environment,
        now: Timestamp,
    ) -> Stage {
        Stage::created(policy, environment, now, Opening::Oneshot)
    }

    /// 同上，造的是会话 `parent` 派出来的第 1 层子会话（施工 7-5）：`session.created` 带 `parent`、`depth`，由父会话造；
    /// 以后说的话都是父会话发的（交代、留言）。
    ///
    /// # Panics
    ///
    /// `parent` 不合会话编号的写法。
    pub fn child(
        policy: impl Fn() -> Policy + 'static,
        environment: Environment,
        now: Timestamp,
        parent: &str,
    ) -> Stage {
        let parent =
            SessionId::parse(parent).unwrap_or_else(|e| panic!("父会话的编号写法坏了：{e}"));
        Stage::created(policy, environment, now, Opening::Child(parent))
    }

    /// 同上，造的是从会话 [`SESSION`] 的日志 `base` 的最后一条分叉出来的子会话（施工 C-8 下）：跑满 `seal_after` 轮封存，
    /// 没有的不封；由 alice 分叉，`session.created` 带 `forked`、`parent`、`depth`。崩了照 `base` 当底子载入。
    ///
    /// # Panics
    ///
    /// `base` 是空的，或者分叉不成（停在一轮当中）。
    pub fn forked(
        policy: impl Fn() -> Policy + 'static,
        environment: Environment,
        now: Timestamp,
        base: &[Event],
        seal_after: Option<NonZeroU32>,
    ) -> Stage {
        Stage::created(
            policy,
            environment,
            now,
            Opening::Fork(base.to_vec(), seal_after),
        )
    }

    /// 照「磁盘」载入：分叉出来的照底子载入。
    pub(super) fn load(&self, at: Timestamp) -> Result<(Session, Vec<Action>), LoadError> {
        let (log, policy, environment) =
            (self.log.clone(), (self.policy)(), self.environment.clone());
        match &self.base {
            Some(base) => Session::load_forked(self.id.clone(), base, log, at, policy, environment),
            None => Session::load(self.id.clone(), log, at, policy, environment),
        }
    }

    fn created(
        policy: impl Fn() -> Policy + 'static,
        environment: Environment,
        now: Timestamp,
        opening: Opening,
    ) -> Stage {
        let mut by: By = serde_json::from_str(r#"{"kind":"person","account":"alice"}"#)
            .unwrap_or_else(|e| panic!("alice 的写法坏了：{e}"));
        let mut created: SessionCreated =
            serde_json::from_str(CREATED).unwrap_or_else(|e| panic!("造会话的写法坏了：{e}"));
        let mut id = SESSION;
        let mut base = None;
        match opening {
            Opening::Main => {}
            Opening::Oneshot => created.oneshot = true,
            Opening::Child(parent) => {
                created.parent = Some(parent.clone());
                created.depth = Some(1);
                by = By::Session(crate::origin::Session { id: parent });
                id = CHILD_SESSION;
            }
            Opening::Fork(events, seal_after) => {
                let parent = SessionId::parse(SESSION).unwrap_or_else(|e| panic!("{e}"));
                let upto = events.last().map(|event| event.seq);
                let upto = upto.unwrap_or_else(|| panic!("分叉要有父会话的日志"));
                created.forked = Some(Forked {
                    from: parent.clone(),
                    upto,
                    seal_after,
                });
                created.parent = Some(parent);
                created.depth = Some(1);
                id = CHILD_SESSION;
                base = Some(events);
            }
        }
        let id = SessionId::parse(id).unwrap_or_else(|e| panic!("替身的会话编号写法坏了：{e}"));
        let (session, actions) = match &base {
            Some(base) => {
                let (cause, at, env) = (command_id(0), now, environment.clone());
                Session::fork(
                    id.clone(),
                    base,
                    cause,
                    by.clone(),
                    at,
                    created,
                    policy(),
                    env,
                )
                .unwrap_or_else(|e| panic!("分叉不成：{e}"))
            }
            None => Session::create(
                id.clone(),
                command_id(0),
                by.clone(),
                now,
                created,
                policy(),
                environment.clone(),
            ),
        };
        let mut stage = Stage {
            session,
            id,
            base,
            policy: Box::new(policy),
            environment,
            log: Vec::new(),
            requests: Vec::new(),
            marks: Vec::new(),
            replies: Vec::new(),
            transients: Vec::new(),
            order: Vec::new(),
            ran: Vec::new(),
            lines: VecDeque::new(),
            plays: VecDeque::new(),
            verdicts: VecDeque::new(),
            injections: VecDeque::new(),
            presents: Vec::new(),
            saids: Vec::new(),
            speakers: Vec::new(),
            held_model: None,
            hold_wakes: false,
            held_wake: None,
            held_tools: Vec::new(),
            stops: Vec::new(),
            now,
            next: 1,
            by,
            limits: None,
            summaries: None,
            disk: super::disk::Disk::default(),
            upward: Vec::new(),
            stopping: Vec::new(),
            recaps: Vec::new(),
            recap_lines: VecDeque::new(),
            held_recap: None,
            titles: Vec::new(),
            title_lines: VecDeque::new(),
            held_title: None,
            prepare: super::prepare::Prepares::default(),
            routing: super::routing::Routing::default(),
            nested: super::nested::Bench::default(),
            sight: super::sight::Sight::default(),
            swap: None,
        };
        stage.settle(actions);
        stage
    }
}
