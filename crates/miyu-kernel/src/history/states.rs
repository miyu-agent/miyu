//! 有效历史另记的一张表：包在这个会话里写过的状态（施工 TW-1，设计 `46-包的会话状态.md`）。
//!
//! 每个包当前的是它最近一份没撤掉的 `package.state`（工具结果的效果、程序直接写的事件一样算）。那一条会被压缩换掉，压缩
//! 以后头和检查点照样要看，所以另记一张表：压缩不丢，撤销、恢复只改「撤掉了没有」，同待办的表（`todos.rs`）。清空上下文的
//! 检查点清掉全部包的。

use std::collections::BTreeMap;

use crate::event::{Body, CompactTrigger, Effect, Event, State};
use crate::id::TurnId;

/// 写过的每一份，照先后。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct States {
    written: Vec<Written>,
}

/// 写过的一份。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Written {
    /// 哪个包的；清空上下文清掉全部的是 `None`。
    package: Option<String>,
    /// 换上的那一份。
    state: State,
    /// 写它的那一轮：撤销、恢复照它改 `undone`。
    turn: Option<TurnId>,
    /// 写它的那一轮撤掉了：恢复了就不算。
    undone: bool,
}

impl States {
    /// 记下这一条带来的变化。
    pub(super) fn note(&mut self, event: &Event) {
        let mut push = |package: Option<String>, state: State| {
            self.written.push(Written {
                package,
                state,
                turn: event.turn,
                undone: false,
            });
        };
        match &event.body {
            Body::ContextCompacted(compacted)
                if compacted.trigger == Some(CompactTrigger::Clear) =>
            {
                push(None, State::default());
            }
            Body::PackageState(written) => {
                push(Some(written.package.clone()), written.state.clone())
            }
            Body::ToolResult(result) => {
                for effect in &result.effects {
                    if let Effect::PackageState(written) = effect {
                        push(Some(written.package.clone()), written.state.clone());
                    }
                }
            }
            Body::TurnReverted(reverted) => self.mark(&reverted.turns, true),
            Body::TurnUnreverted(unreverted) => self.mark(&unreverted.turns, false),
            _ => {}
        }
    }

    /// 每个包当前的状态，照包的编号排：清掉了的、写过的都撤掉了的不在里面。
    pub(super) fn current(&self) -> BTreeMap<String, State> {
        let mut current = BTreeMap::new();
        for written in self.written.iter().filter(|written| !written.undone) {
            match &written.package {
                None => current.clear(),
                Some(package) if written.state.is_empty() => {
                    current.remove(package);
                }
                Some(package) => {
                    current.insert(package.clone(), written.state.clone());
                }
            }
        }
        current
    }

    /// 在这几轮里写的，标成撤掉了没有。
    fn mark(&mut self, turns: &[TurnId], undone: bool) {
        for written in &mut self.written {
            if written.turn.is_some_and(|turn| turns.contains(&turn)) {
                written.undone = undone;
            }
        }
    }
}

/// 这一条会不会动包的状态：写了、清空上下文、撤销、恢复。
pub fn touches(event: &Event) -> bool {
    match &event.body {
        Body::PackageState(_) | Body::TurnReverted(_) | Body::TurnUnreverted(_) => true,
        Body::ContextCompacted(compacted) => compacted.trigger == Some(CompactTrigger::Clear),
        Body::ToolResult(result) => result
            .effects
            .iter()
            .any(|effect| matches!(effect, Effect::PackageState(_))),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
