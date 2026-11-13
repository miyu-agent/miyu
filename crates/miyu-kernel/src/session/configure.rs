//! 会话里换模型（施工 8-10，`docs/blueprint/models.md`「怎么走」第六条，`kernel/session.md`「换模型」）、换思考强度（施工
//! SM-1，设计 51，「怎么走」第十一条）：人换，内核记；和现在的一样的不记，下一个回合开始时生效。
//!
//! 会话的引用从日志算：`session.created` 的 `model`，被后来带 `model` 的 `session.policy_changed` 盖掉，撤掉的回合里的也算
//! （换模型不是对话的一部分，照改标题）。内核只存字，不解读：回合开始时交给执行器照这一轮的配置重新解析，钉着的没了、
//! 执行器退回了默认的，内核在注入前面记一条，`by` 是内核。
//!
//! 思考强度一样从日志算（施工 SM-1）：每条带 `effort` 的 `session.policy_changed` 换会话的那一档，`null` 是回到跟着配置；
//! 撤掉的回合里的也算，换模型不清它。施工 8-18 那时记的每个模型一格（带 `model`）不认（`event/session.rs` 的
//! `Effort::session`）。回合开始时执行器照 [`Session::effort`] 取，一次请求用不用它由执行器照真发的那个模型的档位定。
//!
//! 上下文上限照同一个办法从日志算（施工 CTX-1，设计 68 第四节）：每条带 `context` 的 `session.policy_changed` 换上限，`null`
//! 是去掉；撤掉的回合里的也算，换模型不清它。执行器照 [`Session::context`] 交给路由，生效的窗口由路由取小的。
//!
//! 钉的看图模型也从日志算（施工 VM-1，`models.md`「怎么走」第十三条第 4 条）：每条带 `vision` 的 `session.policy_changed`
//! 换会话钉的那一个，空的是回到跟着 `models.vision`；撤掉的回合里的也算，换模型不清它。回合开始时执行器照
//! [`Session::vision`] 交给路由，替看图照它发。
//!
//! 钉的压缩触发线照同一个办法从日志算（施工 CT-1，`compaction.md` 第二条第 2 条）：每条带 `compaction_line` 的
//! `session.policy_changed` 换会话钉的百分数，`null` 是回到跟着 `compaction.line`；撤掉的回合里的也算，换模型不清它。回合开始
//! 时会话那一层照 [`Session::pinned_line`]（没钉的照配置）换策略快照里的数，换了的记成换快照。

use super::action::Action;
use super::{Session, accepted};
use crate::event::{Body, ContextCap, Effort, Event, LinePin, PolicyChanged};
use crate::id::{CommandId, Seq};
use crate::origin::By;
use crate::time::Timestamp;

/// 会话的引用，和最近一次换模型写在第几条；会话现在要用的人格、预设（施工 P-13）：每追加一条记一次，载入时照整份日志一路算
/// （`load.rs`）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Reference {
    /// 人格：`session.created` 的，被后来带 `persona` 的 `session.policy_changed` 盖掉（空的是无人格）。
    persona: Option<String>,
    /// 预设：同上。
    preset: Option<String>,
    /// 会话现在的引用；以前的日志没有记下的、造的时候连 `models.chat` 都没配的没有。
    model: Option<String>,
    /// 最近一条带 `model` 的 `session.policy_changed` 的序号：写在它前面的自动压缩的暂停、失败不再算（`compaction.md`
    /// 第十条第 6 条，「施工时定的」8-10）。没换过的没有。只换思考强度的不算。
    changed: Option<Seq>,
    /// 会话记的思考强度（施工 SM-1）：没记的、回到跟着配置的没有。
    effort: Option<String>,
    /// 会话的上下文上限，token 数（施工 CTX-1）：没设的、去掉了的没有。
    context: Option<u64>,
    /// 会话钉的看图模型（施工 VM-1）：模型或 `@池`；没钉的、去掉了的没有（跟着 `models.vision`）。
    vision: Option<String>,
    /// 会话钉的压缩触发线，窗口的百分之几（施工 CT-1）：没钉的、去掉了的没有（跟着 `compaction.line`）。
    line: Option<u64>,
    /// 换了预设、还没告诉她（施工 DM-8，`facts/preset.rs`）：换之前的那个预设（空的是不用预设）。内核注入了预设那一块就清掉；
    /// 换了几次还没开回合的，记最早的那个，换回它的也清掉。
    announce: Option<String>,
}

impl Reference {
    /// 记下一条：`session.created` 定开头的引用、人格、预设，带 `model`、`persona`、`preset` 的 `session.policy_changed` 换掉它们。
    pub(super) fn note(&mut self, event: &Event) {
        match &event.body {
            Body::SessionCreated(created) => {
                self.model.clone_from(&created.model);
                self.persona.clone_from(&created.persona);
                // 明着选了无工具的记成空的预设，同换预设时写的空编号（施工 P-16 下）。
                self.preset = match created.no_tools {
                    true => Some(String::new()),
                    false => created.preset.clone(),
                };
                self.announce = None;
            }
            // 内核告诉过她换了预设（施工 DM-8）。
            Body::ContextInjected(fact)
                if event.by == By::Kernel && fact.kind.as_str() == crate::facts::PRESET =>
            {
                self.announce = None;
            }
            // 空的是回到跟着 `models.chat`（施工 8-10 再补）。
            Body::PolicyChanged(PolicyChanged {
                model: Some(model), ..
            }) => {
                self.model = Some(model.clone()).filter(|model| !model.is_empty());
                self.changed = Some(event.seq);
            }
            _ => {}
        }
        // 换人格、预设（施工 P-13）：和换模型可以在同一条里。
        if let Body::PolicyChanged(changed) = &event.body {
            if let Some(persona) = &changed.persona {
                self.persona = Some(persona.clone()).filter(|persona| !persona.is_empty());
            }
            if let Some(preset) = &changed.preset {
                let before = self.preset.replace(preset.clone()).unwrap_or_default();
                self.announce = match self.announce.take() {
                    Some(first) if first == *preset => None,
                    Some(first) => Some(first),
                    None => Some(before),
                };
            }
            // 换思考强度（施工 SM-1）：和换模型可以在同一条里，8-18 那时的不认。
            if let Some(level) = changed.effort.as_ref().and_then(Effort::session) {
                self.effort = level.map(str::to_string);
            }
            // 换上下文上限（施工 CTX-1）：同上，可以和别的一起。
            if let Some(cap) = &changed.context {
                self.context = cap.tokens;
            }
            // 钉看图模型（施工 VM-1）：空的是去掉，同换模型。
            if let Some(vision) = &changed.vision {
                self.vision = Some(vision.clone()).filter(|vision| !vision.is_empty());
            }
            // 钉压缩触发线（施工 CT-1）：同换上下文上限。
            if let Some(pin) = &changed.compaction_line {
                self.line = pin.percent;
            }
        }
    }

    /// 会话现在的引用。
    pub(super) fn current(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// 会话现在要用的人格：无人格的没有。
    pub(super) fn persona(&self) -> Option<&str> {
        self.persona.as_deref()
    }

    /// 会话现在要用的预设：以前的日志没记的没有，无工具的是空的（施工 P-16 下）。
    pub(super) fn preset(&self) -> Option<&str> {
        self.preset.as_deref()
    }

    /// 会话记的思考强度：没记的没有。
    pub(super) fn effort(&self) -> Option<&str> {
        self.effort.as_deref()
    }

    /// 会话的上下文上限：没设的没有。
    pub(super) fn context(&self) -> Option<u64> {
        self.context
    }

    /// 会话钉的看图模型：没钉的没有。
    pub(super) fn vision(&self) -> Option<&str> {
        self.vision.as_deref()
    }

    /// 会话钉的压缩触发线：没钉的没有。
    pub(super) fn line(&self) -> Option<u64> {
        self.line
    }

    /// 换了预设、还没告诉她的：换之前的那个预设（空的是不用预设）。没换过、告诉过的没有（施工 DM-8）。
    pub(super) fn announce(&self) -> Option<&str> {
        self.announce.as_deref()
    }

    /// 第 `seq` 条写在最近一次换模型后面；没换过的都算。熔断照它只看换过去以后的暂停、失败。
    pub(super) fn after_change(&self, seq: Seq) -> bool {
        self.changed.is_none_or(|changed| seq > changed)
    }
}

impl Session {
    /// 会话现在的引用：模型或 `@池`（施工 8-10）。`session.created` 的 `model`，被后来带 `model` 的 `session.policy_changed`
    /// 盖掉，撤掉的回合里的也算；以前的日志没有的、造的时候连 `models.chat` 都没配的没有（跟着 `models.chat`）。只读：执行器
    /// 载入时照它造路由（`session/actor.md` 第 2 条）。
    pub fn reference(&self) -> Option<&str> {
        self.reference.current()
    }

    /// 换模型：和现在的一样的接受，什么都不记，编号照样记下；不一样的记一条 `session.policy_changed`，`by` 是换的人，回合
    /// 进行中的带上这个回合，空闲时没有。`model` 空的是回到跟着 `models.chat`（施工 8-10 再补），本来就跟着的什么都不记。什么
    /// 时候来都收，读回日志的时候照「命令和回应」第 5 条拒（`session.rs`）。
    pub(super) fn configure(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        model: String,
    ) -> Vec<Action> {
        if self.reference() == Some(model.as_str()).filter(|model| !model.is_empty()) {
            self.recent.insert(id.clone(), Vec::new());
            return vec![accepted(id, Vec::new())];
        }
        let body = Body::PolicyChanged(PolicyChanged {
            model: Some(model),
            ..PolicyChanged::default()
        });
        let event = self.record(at, by, Some(id.clone()), body);
        self.accept(id, vec![event.seq]);
        vec![Action::Append(vec![event])]
    }

    /// 会话记的思考强度（施工 SM-1，设计 51）：带 `effort` 的 `session.policy_changed` 里最后那一档，撤掉的回合里的也算；
    /// 没记的、回到跟着配置的没有（照配置）。只读：执行器载入时照它造路由，回合开始时交给路由；这个模型有没有这一档由路由查。
    pub fn effort(&self) -> Option<&str> {
        self.reference.effort()
    }

    /// 换思考强度（施工 SM-1）：`level` 是 `None` 的回到跟着配置。和现在的一样的接受，什么都不记，编号照样记下；不一样的记
    /// 一条 `session.policy_changed`，只写 `effort`，`by` 是换的人，回合进行中的带上这个回合。档位协议那一头查过。
    pub(super) fn set_effort(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        level: Option<String>,
    ) -> Vec<Action> {
        if self.effort() == level.as_deref() {
            self.recent.insert(id.clone(), Vec::new());
            return vec![accepted(id, Vec::new())];
        }
        let body = Body::PolicyChanged(PolicyChanged {
            effort: Some(Effort { model: None, level }),
            ..PolicyChanged::default()
        });
        let event = self.record(at, by, Some(id.clone()), body);
        self.accept(id, vec![event.seq]);
        vec![Action::Append(vec![event])]
    }

    /// 会话的上下文上限，token 数（施工 CTX-1，设计 68）：带 `context` 的 `session.policy_changed` 里最后那一个，撤掉的回合里的
    /// 也算；没设的、去掉了的没有（照模型的窗口）。只读：执行器载入时照它造路由，变了交给路由，生效的窗口由路由取小的。
    pub fn context(&self) -> Option<u64> {
        self.reference.context()
    }

    /// 换上下文上限（施工 CTX-1）：`tokens` 是 `None` 的去掉。和现在的一样的接受，什么都不记，编号照样记下；不一样的记一条
    /// `session.policy_changed`，只写 `context`，`by` 是换的人，回合进行中的带上这个回合。太小的协议那一头拒过。
    pub(super) fn set_context(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        tokens: Option<u64>,
    ) -> Vec<Action> {
        if self.context() == tokens {
            self.recent.insert(id.clone(), Vec::new());
            return vec![accepted(id, Vec::new())];
        }
        let body = Body::PolicyChanged(PolicyChanged {
            context: Some(ContextCap { tokens }),
            ..PolicyChanged::default()
        });
        let event = self.record(at, by, Some(id.clone()), body);
        self.accept(id, vec![event.seq]);
        vec![Action::Append(vec![event])]
    }

    /// 会话钉的看图模型（施工 VM-1）：带 `vision` 的 `session.policy_changed` 里最后那一个，撤掉的回合里的也算；没钉的、去掉
    /// 了的没有（照 `models.vision`）。只读：执行器载入时照它造路由，回合开始时交给路由，替看图照它发；也写进给头看的会话
    /// 状态。
    pub fn vision(&self) -> Option<&str> {
        self.reference.vision()
    }

    /// 钉看图模型（施工 VM-1）：`vision` 是 `None` 的去掉。和现在的一样的接受，什么都不记，编号照样记下；不一样的记一条
    /// `session.policy_changed`，只写 `vision`（去掉的写空的，同换模型），`by` 是换的人，回合进行中的带上这个回合。引用协议
    /// 那一头查过。
    pub(super) fn set_vision(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        vision: Option<String>,
    ) -> Vec<Action> {
        if self.vision() == vision.as_deref() {
            self.recent.insert(id.clone(), Vec::new());
            return vec![accepted(id, Vec::new())];
        }
        let body = Body::PolicyChanged(PolicyChanged {
            vision: Some(vision.unwrap_or_default()),
            ..PolicyChanged::default()
        });
        let event = self.record(at, by, Some(id.clone()), body);
        self.accept(id, vec![event.seq]);
        vec![Action::Append(vec![event])]
    }

    /// 会话钉的压缩触发线，窗口的百分之几（施工 CT-1）：带 `compaction_line` 的 `session.policy_changed` 里最后那一个，撤掉的回合里的也算；
    /// 没钉的、去掉了的没有（照 `compaction.line`）。只读：回合开始时会话那一层照它换策略快照里的数；也写进给头看的会话状态。
    pub fn pinned_line(&self) -> Option<u64> {
        self.reference.line()
    }

    /// 钉压缩触发线（施工 CT-1）：`percent` 是 `None` 的去掉。和现在的一样的接受，什么都不记，编号照样记下；不一样的记一条
    /// `session.policy_changed`，只写 `compaction_line`，`by` 是换的人，回合进行中的带上这个回合。范围协议那一头查过。
    pub(super) fn set_line(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        percent: Option<u64>,
    ) -> Vec<Action> {
        if self.pinned_line() == percent {
            self.recent.insert(id.clone(), Vec::new());
            return vec![accepted(id, Vec::new())];
        }
        let body = Body::PolicyChanged(PolicyChanged {
            compaction_line: Some(LinePin { percent }),
            ..PolicyChanged::default()
        });
        let event = self.record(at, by, Some(id.clone()), body);
        self.accept(id, vec![event.seq]);
        vec![Action::Append(vec![event])]
    }

    /// 回合开始时执行器退回了默认（施工 8-10）：`from` 正是现在的引用、`to` 和它不一样的，记一条 `session.policy_changed`
    /// （`model` 是退回的，`replaced` 是原来的），`by` 是内核，`cause` 是回合的。对不上的不理：执行器交回来之前人又换了，
    /// 人换的算数，下一轮再解析（「施工时定的」8-10）。
    pub(super) fn fall_back(
        &mut self,
        at: Timestamp,
        cause: Option<CommandId>,
        replaced: Option<Replaced>,
    ) -> Option<Event> {
        let Replaced { from, to } = replaced?;
        if self.reference() != Some(from.as_str()) || from == to {
            return None;
        }
        let body = Body::PolicyChanged(PolicyChanged {
            model: Some(to),
            replaced: Some(from),
            ..PolicyChanged::default()
        });
        Some(self.record(at, By::Kernel, cause, body))
    }
}

/// 钉着的引用没了、执行器退回了默认（施工 8-10）：交回 [`super::Input::TurnStartHooksDone`] 的 `replaced`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replaced {
    /// 原来的引用：回合开始时内核交出去的那一个。
    pub from: String,
    /// 退回的引用：这一轮的 `models.chat`。
    pub to: String,
}
