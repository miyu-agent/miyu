//! 人用 `!` 打的命令跑出来的样子（施工 SH-1 下，设计 57 第四节第 4 条）：挂在那条 `user` 下面的一格 `shell`。

use serde::{Deserialize, Serialize};

use miyu_kernel::event::ShellRan;

/// 那条命令：还没跑完的只有 `running`；跑完了的照 `shell.ran`。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Shell {
    /// 还没跑完（排着的也是：头照那条 `user` 的 `queued` 分）。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub running: bool,
    /// 退出码。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<i32>,
    /// 杀掉它的信号（Unix）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<u32>,
    /// 输出，照事件原样（截过的中间有一行省了多少）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// 截掉了几个字。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub omitted: Option<u64>,
    /// 异常结束的为什么：`timed_out`、`interrupted`、`aborted`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// 用时，毫秒。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

impl Shell {
    /// 还没跑完的。
    pub fn running() -> Shell {
        Shell {
            running: true,
            ..Shell::default()
        }
    }

    /// 跑完了的。
    pub fn ran(ran: &ShellRan) -> Shell {
        Shell {
            running: false,
            exit: ran.exit,
            signal: ran.signal,
            text: Some(ran.text.clone()),
            omitted: Some(ran.omitted),
            reason: ran
                .reason
                .as_ref()
                .map(|reason| reason.as_str().to_string()),
            duration_ms: Some(ran.duration_ms),
        }
    }
}
