//! 内核的嵌套调用动作（施工 M-2，蓝图 `kernel/session.md`「嵌套调用」）：判照直接调用的那条链、在阻塞线程里判；要问人、工具要
//! 看一眼的照直接调用先看（施工 BS-1 中：会话里的 `miyu pkg` 装包，卡片照看一眼的那一份）；跑、掐、答交给执行工具的端口
//! （`tools/nested.rs`）。

use std::sync::Arc;

use miyu_kernel::session::{Input, NestedAction, NestedInput};

use super::Actor;
use crate::blocking::blocking;
use crate::tools::NestedRun;

impl Actor {
    /// 回一个嵌套调用的动作；当场就能回的，交回要送进内核的输入。
    pub(super) async fn nested(&mut self, action: NestedAction) -> Option<Input> {
        match action {
            NestedAction::Guard {
                id,
                name,
                args,
                cwd,
                dirs,
                permission,
                grants,
            } => {
                let guard = Arc::clone(&self.guard);
                let (asked, looked) = ((name.clone(), args.clone()), cwd.clone());
                let (verdict, checked) =
                    blocking(move || guard.judge(&name, args, cwd, &dirs, &permission, &grants))
                        .await;
                // 判过的真实位置跑的时候交给工具（施工 5-18）。
                self.tools.checked_nested(&id, checked);
                let verdict = self
                    .tools
                    .previewed(&asked.0, asked.1, looked, verdict)
                    .await;
                Some(Input::Nested(NestedInput::Guarded {
                    at: self.clock.now(),
                    id,
                    verdict,
                }))
            }
            NestedAction::Run {
                id,
                name,
                args,
                cwd,
                dirs,
                permission,
            } => {
                let usage = crate::usage::asked(
                    &name,
                    &self.session,
                    self.config.current(),
                    self.clock.now(),
                );
                let run = NestedRun {
                    id,
                    name,
                    args,
                    cwd,
                    dirs,
                    permission,
                    turn: self.session.turn_cause().map(|(turn, _)| turn),
                    compacted: self.session.compacted_upto(),
                    config: Arc::clone(self.config.current()),
                    usage,
                };
                self.tools.run_nested(self.clock.now(), run)
            }
            NestedAction::Cancel { id } => {
                self.tools.cancel_nested(&id);
                None
            }
            NestedAction::Answer { id, answer } => {
                self.tools.answer_nested(&id, answer);
                None
            }
        }
    }
}
