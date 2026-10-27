//! 一个扩展的状态（施工 9-4 上；施工 IM-1 中从 `extensions.rs` 挪出来：那边到了行数上限）：在哪一步、为什么停下、连续失败了
//! 几次，和表、看管的任务一起拿着的那一格。

use std::sync::{Mutex, PoisonError};

use tokio::sync::broadcast;

/// 一个扩展这时在哪一步。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    /// 关着。
    Off,
    /// 要拉起、拉起了，还没握手。
    Starting {
        /// 进程号：还没起来的没有。
        pid: Option<u32>,
    },
    /// 握了手。
    Running {
        /// 进程号：网址的 MCP 服务没有进程，没有（施工 X-32 上）。
        pid: Option<u32>,
    },
    /// 退避中：到点再拉起。
    Waiting {
        /// 哪一刻拉起。
        until: tokio::time::Instant,
    },
    /// 不再拉起：开关照旧开着，`restart` 再试，核心下次起来也再试。
    Stopped(Reason),
}

impl State {
    /// 协议里的写法。
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            State::Off => "off",
            State::Starting { .. } => "starting",
            State::Running { .. } => "running",
            State::Waiting { .. } => "waiting",
            State::Stopped(_) => "stopped",
        }
    }

    /// 在干活、或者等着再干：核心不算空闲。
    pub(crate) fn busy(self) -> bool {
        matches!(
            self,
            State::Starting { .. } | State::Running { .. } | State::Waiting { .. }
        )
    }
}

/// 为什么停下了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reason {
    /// 退出码 1：配置错、端口被占这类重启也没用的。
    ConfigError,
    /// 连续失败到了上限。
    FailedRepeatedly,
    /// 程序不在 `miyu` 旁边。
    NotInstalled,
    /// 起不来：程序在，系统不让起、建不了它的目录和标准错误的文件。
    CannotStart,
    /// 清单说的协议版本不包含核心的。
    ProtocolMismatch,
    /// 要的能力还有没批的（施工 9-4 下上）：批了（`extension.enable` 带 `approve`）才拉起。
    NeedsApproval,
    /// 网址的 MCP 服务要浏览器登录（施工 X-42 上）：人登录了（`mcp.login`）、令牌存进密钥文件才重连。
    NeedsLogin,
}

impl Reason {
    /// 协议里的写法。
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Reason::ConfigError => "config_error",
            Reason::FailedRepeatedly => "failed_repeatedly",
            Reason::NotInstalled => "not_installed",
            Reason::CannotStart => "cannot_start",
            Reason::ProtocolMismatch => "protocol_mismatch",
            Reason::NeedsApproval => "needs_approval",
            Reason::NeedsLogin => "needs_login",
        }
    }
}

/// 一个扩展的状态和连续失败了几次：看管的任务写，列状态时读。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Status {
    /// 在哪一步。
    pub(crate) state: State,
    /// 连续失败了几次。
    pub(crate) failures: u32,
}

/// 表和看管的任务一起拿着的那一份：状态，改了就广播是哪个包变了（施工 9-4 补，订阅的连接照它推 `extension.changed`）。
pub(crate) struct Cell {
    /// 包的编号。
    pub(super) id: String,
    /// 状态。
    pub(super) status: Mutex<Status>,
    /// 谁变了：广播包的编号。
    pub(super) changed: broadcast::Sender<String>,
}

impl Cell {
    /// 这时的状态。
    pub(crate) fn get(&self) -> Status {
        *self.status.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 记下状态，广播一声。
    pub(crate) fn set(&self, state: State, failures: u32) {
        *self.status.lock().unwrap_or_else(PoisonError::into_inner) = Status { state, failures };
        if self.changed.send(self.id.clone()).is_err() {
            // 没有订阅着的连接：不用说。
        }
    }
}
