//! 人敲的 `/loop` 那一行（施工 FX-15，`internal/blueprint/schedules.md` 第五节第 9 条，2026-11-22 项目主人定：「我发什么消息，
//! 显示就该是什么消息」）：敲命令那一刻照会话记下，交第一次（`fire.rs` 的 `kickoff`）时取走，记进 `message.user` 的 `command`，
//! 头照它显示「/loop …」。一个会话一个循环，再敲一次换掉。只在内存里：核心在这中间重启过的，第一次照原话交、不带。

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use miyu_kernel::event::TypedCommand;
use miyu_kernel::id::SessionId;

use super::Schedules;

/// `/loop` 这条命令的名字。
const LOOP: &str = "loop";

/// 等着交第一次的：会话 → 人敲的那一行。
#[derive(Default)]
pub(super) struct Typed(Mutex<BTreeMap<SessionId, String>>);

impl Typed {
    fn lock(&self) -> MutexGuard<'_, BTreeMap<SessionId, String>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Schedules {
    /// 人在会话 `session` 里敲了 `/loop`，整一行是 `text`：建那一行循环之前记下（建了马上就到点）。
    pub(crate) fn typed_loop(&self, session: &SessionId, text: &str) {
        self.typed.lock().insert(session.clone(), text.to_string());
    }

    /// 交会话 `session` 的循环的第一次：取走记下的那一行；没有的没有。
    pub(super) fn take_typed(&self, session: &SessionId) -> Option<TypedCommand> {
        let text = self.typed.lock().remove(session)?;
        Some(TypedCommand {
            name: LOOP.to_string(),
            text,
        })
    }
}

/// 照会话认的那一行是不是 `/loop` 敲出来的（撤销时认，`undone.rs`）。
pub(super) fn is_loop(command: &TypedCommand) -> bool {
    command.name == LOOP
}
