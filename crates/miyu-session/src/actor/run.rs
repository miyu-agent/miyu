//! 派一次工具调用（从 `actor.rs` 挪来，施工 M-3：那边到了 500 行）：照内核的 `RunTool` 抄好这一刻的几样，交给执行工具的端口。

use std::sync::Arc;

use miyu_kernel::session::{Action, Input};

use super::Actor;
use crate::agents::Inherit;
use crate::jobs::JobLimits;
use crate::tools::Dispatch;

impl Actor {
    /// 回内核的 `RunTool`：别的动作不归这里，不回。
    pub(super) fn run_tool(&mut self, action: Action) -> Option<Input> {
        let Action::RunTool {
            call_id,
            name,
            args,
            cwd,
            dirs,
            permission,
            grants,
            cause,
            asked,
        } = action
        else {
            return None;
        };
        let at = self.clock.now();
        // 后台运行关着的会话不给任务端口（施工 T-1 上）：`shell` 要放到后台的照「这里不能放到后台」说。任务的上限照这一轮
        // 的配置（施工 7-12 下）。
        let (config, persona) = (self.config.current(), self.persona.as_ref());
        let background = !persona.is_some_and(|refresh| refresh.snapshot.foreground);
        let jobs = background.then(|| self.jobs.port(call_id, cause, JobLimits::of(config)));
        let usage = crate::usage::asked(&name, &self.session, self.config.current());
        self.tools.run(
            at,
            Dispatch {
                call_id,
                name,
                args,
                cwd,
                dirs,
                permission,
                grants,
                jobs,
                subagents: self.session.subagents(),
                inherit: Inherit::of(&*self.model),
                usage,
                turn: self.session.turn_cause().map(|(turn, _)| turn),
                compacted: self.session.compacted_upto(),
                asked,
                config: Arc::clone(self.config.current()),
                // 脚本里能调的（施工 M-3）：一件都没有的不开通道。
                clicall: self.session.clicall_tools(),
            },
        )
    }
}
