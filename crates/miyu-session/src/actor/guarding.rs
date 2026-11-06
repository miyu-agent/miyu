//! 扩展的守卫（施工 HK-1，设计 `65-工具挂接点.md` 第三节，蓝图 `kernel/asking.md`「执行前的链」）：权限策略判完、没拒绝的
//! 调用，先查一次表（[`miyu_tool::HooksPort::guards`]）；没有挂在它上面的守卫，结论当场送进内核，照原来的路走，一点不多。
//! 有的起一个任务问（端口并行问、各自限时、照包的编号合），答了再照「最严者胜」合上策略的结论，经执行器的回报送回内核
//! （[`Back::Guarded`]）。actor 不等它：打断、插话随时进得来（设计 05 第五节规则 3）。
//!
//! 在等的调用一有了结果（打断、急着插话、收紧成只读、重启时内核补的那一条，`tool.result`、`tool.nested`；追加时看，`store.rs`），它的任务就掐掉：
//! 端口的 future 被丢掉，还没答的守卫收到 `guard.cancel`（第三节第 6 条）。actor 退出时一起掐。

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{CallId, NestedId};
use miyu_kernel::session::{Input, NestedInput, Verdict};
use miyu_kernel::time::Timestamp;
use miyu_kernel::tool::Access;
use miyu_tool::{GuardAsk, Guarded, Where};
use tokio::task::AbortHandle;

use super::Actor;
use super::back::answer_back;
use crate::port::Back;

/// 在等守卫的一次调用：直接调的，脚本里调的（设计 65 第三节第 7 条）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Waiting {
    /// 直接调的：调用编号。
    Call(CallId),
    /// 脚本里调的：嵌套调用的编号。
    Nested(NestedId),
}

impl Waiting {
    /// 问守卫时交的 `call_id`。
    fn label(&self) -> String {
        match self {
            Waiting::Call(id) => id.to_string(),
            Waiting::Nested(id) => id.to_string(),
        }
    }
}

/// 在等守卫的调用和问它们的任务：拿掉一个就掐掉那个任务，整份放下时全掐。
#[derive(Default)]
pub(crate) struct Guarding(Vec<(Waiting, AbortHandle)>);

impl Guarding {
    /// 拿掉 `waiting` 的那一个，掐掉它的任务（答完了的掐了也没事）。
    fn drop_one(&mut self, waiting: &Waiting) {
        self.0.retain(|(one, task)| {
            let keep = one != waiting;
            if !keep {
                task.abort();
            }
            keep
        });
    }

    /// 这一批要追加的事件里有在等守卫的调用的结果的：不等了。
    pub(crate) fn settled(&mut self, events: &[Event]) {
        if self.0.is_empty() {
            return;
        }
        for event in events {
            match &event.body {
                Body::ToolResult(result) => self.drop_one(&Waiting::Call(result.call_id)),
                Body::ToolNested(nested) => self.drop_one(&Waiting::Nested(nested.id.clone())),
                _ => {}
            }
        }
    }
}

impl Drop for Guarding {
    fn drop(&mut self) {
        for (_, task) in &self.0 {
            task.abort();
        }
    }
}

impl Actor {
    /// 权限策略判完了 `waiting`（工具 `name`、修正过的参数 `args`、这一轮的工作目录 `cwd`），结论是 `verdict`：拒绝的、没挂着
    /// 守卫的交回它，当场送进内核；挂着的起任务问，交回没有，答了经 [`Back::Guarded`] 送回来。
    pub(super) fn guarded(
        &mut self,
        waiting: Waiting,
        (name, args, cwd): (&str, &str, &str),
        verdict: Verdict,
    ) -> Option<Verdict> {
        // 策略拒绝的到此为止，不问扩展（设计 65 第八节第 1 条）；目录里没有的执行时报用不了，不问。
        if matches!(verdict, Verdict::Deny { .. }) {
            return Some(verdict);
        }
        let Some(access) = self.guard.access_of(name) else {
            return Some(verdict);
        };
        let Some((port, session)) = self.tools.hooks() else {
            return Some(verdict);
        };
        if !port.guards(name, access.as_str()) {
            return Some(verdict);
        }
        let caller = self
            .tools
            .caller_of(self.session.turn_cause().map(|(turn, _)| turn));
        let venue = self.place().unwrap_or(match caller.venue.as_str() {
            crate::agents::LOCAL => Where::Local,
            _ => Where::Private,
        });
        let ask = GuardAsk {
            session: session.as_str().to_string(),
            call_id: waiting.label(),
            tool: name.to_string(),
            args: args.to_string(),
            access: access.as_str().to_string(),
            cwd: cwd.to_string(),
            owner: caller.owner,
            venue,
        };
        let backs = self.backs.clone();
        let back = waiting.clone();
        let task = tokio::spawn(async move {
            let verdict = merged(verdict, port.guard(ask).await, access);
            answer_back(
                &backs,
                Back::Guarded {
                    waiting: back,
                    verdict,
                },
            );
        });
        self.guarding.0.push((waiting, task.abort_handle()));
        None
    }

    /// 守卫答了：拿掉它，写成内核的输入。内核只收还在等结论的（`kernel/asking.md`「执行前的链」第 2 条），迟到的照常不理。
    pub(super) fn guards_answered(
        &mut self,
        at: Timestamp,
        waiting: Waiting,
        verdict: Verdict,
    ) -> Input {
        self.guarding.drop_one(&waiting);
        match waiting {
            Waiting::Call(call_id) => Input::ToolGuarded {
                at,
                call_id,
                verdict,
            },
            Waiting::Nested(id) => Input::Nested(NestedInput::Guarded { at, id, verdict }),
        }
    }
}

/// 守卫的结论合上策略的（设计 65 第三节第 4 条）：最严者胜，只能收紧。拒绝的拒绝；策略要问人的照旧问人（它带着放行规则）；
/// 策略放行、守卫要问人的，问人，`by` 是那个包，不提规则（只能允许这一次或者拒绝）；别的照策略的。
pub(crate) fn merged(policy: Verdict, guarded: Guarded, access: Access) -> Verdict {
    match (policy, guarded) {
        (_, Guarded::Deny { module, worded }) => Verdict::Deny {
            module,
            text: worded.text,
            human: worded.said,
        },
        (Verdict::Allow, Guarded::Ask { module, detail }) => Verdict::Ask {
            module,
            access,
            rule: None,
            detail,
        },
        (policy, _) => policy,
    }
}

#[cfg(test)]
mod tests;
