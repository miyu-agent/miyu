//! 定时和循环的端口（施工 J-2，设计 `45-定时和循环.md` 第三节第 3、4 条）：`cron_create`、`cron_delete`、`cron_list`、
//! `schedule_wakeup` 经它建、删、列这个会话上的定时任务，改动态循环下一次醒的时刻。实现在协议端点：唤醒表（施工 J-1）
//! 在端点里，造会话、载入时交进来，同 [`crate::PackagesPort`]。

use std::fmt;
use std::future::Future;
use std::pin::Pin;

use miyu_kernel::id::SessionId;
use miyu_kernel::time::Timestamp;

/// 建定时任务的工具名。
pub const CRON_CREATE: &str = "cron_create";

/// 删定时任务的工具名。
pub const CRON_DELETE: &str = "cron_delete";

/// 列定时任务的工具名。
pub const CRON_LIST: &str = "cron_list";

/// 动态循环定下一次的工具名：工具面上有它的会话才认 `/loop`。
pub const SCHEDULE_WAKEUP: &str = "schedule_wakeup";

/// 一个会话里她最多同时挂几个定时任务（设计 45 第三节第 3 条）：照谁建的数，超了报错叫她先删。
pub const PER_SESSION: usize = 20;

/// 一共最多挂几个定时任务（施工 J-3 上，项目主人 2026-10-27 定）：人建、她建、包建合计，熔断性质的安全上限；到了只拒新建，
/// 已有的照跑。
pub const IN_ALL: usize = 200;

/// 动态循环最短隔多久再醒（秒）：短了照它。
pub const WAKEUP_MIN: u64 = 60;

/// 动态循环最长隔多久再醒（秒）：长了照它。
pub const WAKEUP_MAX: u64 = 3600;

/// 她要做的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleAsk {
    /// 在这个会话上建一个：五段 cron、到点交进来的话、是不是循环。
    Create {
        /// 标准五段 cron，照这台机器的时区。
        cron: String,
        /// 到点交进会话的话。
        prompt: String,
        /// 循环的每次到点都跑；不是的跑一次就删。
        recurring: bool,
    },
    /// 删这个会话上的一个。
    Delete {
        /// 编号。
        id: String,
    },
    /// 列这个会话上的。
    List,
    /// 动态循环过多少秒再醒：已经照 [`WAKEUP_MIN`]、[`WAKEUP_MAX`] 收过边界。
    Wakeup {
        /// 过多少秒。
        delay: u64,
        /// 醒来交进来的话，覆盖原来的；不写的照旧。
        prompt: Option<String>,
    },
    /// 停掉这个会话的动态循环。
    StopLoop,
}

/// 唤醒表里的一项，给她看的那几样。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheduled {
    /// 编号：`t1`、`t2`……
    pub id: String,
    /// 什么时候，照建的写法：cron 原文、`every <秒>s`、某个时刻。
    pub when: String,
    /// 到点交进来的话。
    pub prompt: String,
    /// 是不是循环。
    pub recurring: bool,
    /// 循环的哪一种：`fixed`、`dynamic`；不是 `/loop` 建的没有。
    pub kind: Option<&'static str>,
    /// 下一次的时刻；停着的（动态循环醒过、她还没定下一次）没有。
    pub next: Option<Timestamp>,
    /// 人暂停了。
    pub paused: bool,
}

/// 端口答的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleAnswer {
    /// 建好的那一项。
    Created(Scheduled),
    /// 删掉的那一项。
    Deleted(Scheduled),
    /// 这个会话上的，照编号排。
    Listed(Vec<Scheduled>),
    /// 动态循环改好的那一行。
    Woken(Scheduled),
    /// 动态循环停了。
    LoopStopped,
}

/// 没成的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleRefused {
    /// 她在这个会话里已经挂满了 [`PER_SESSION`] 个。
    Full,
    /// 一共已经挂满了 [`IN_ALL`] 个（施工 J-3 上）。
    AllFull,
    /// cron 写法不对：唤醒表说的原话。
    BadCron(String),
    /// 这个会话上没有这个编号。
    NotFound,
    /// 这个会话没开着动态循环。
    NoLoop,
    /// 唤醒表用不了、会话停了：没办成。
    Unavailable,
}

/// 等端口答的 future。
pub type ScheduleAnswering<'a> =
    Pin<Box<dyn Future<Output = Result<ScheduleAnswer, ScheduleRefused>> + Send + 'a>>;

/// 定时和循环的端口：协议端点造一个，交给会话。
pub trait SchedulePort: Send + Sync {
    /// 在会话 `session` 上办 `ask`。
    fn schedule(&self, session: &SessionId, ask: ScheduleAsk) -> ScheduleAnswering<'_>;
}

impl fmt::Debug for dyn SchedulePort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SchedulePort")
    }
}

/// 两个端口比的是不是同一个：[`crate::Call`] 照格子比较时用。
impl PartialEq for dyn SchedulePort {
    fn eq(&self, other: &dyn SchedulePort) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

impl Eq for dyn SchedulePort {}
