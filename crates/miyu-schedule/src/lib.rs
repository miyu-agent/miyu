//! 定时任务、循环这两个软件包（施工 J-2，设计 `45-定时和循环.md` 第三节第 3、4 条）：她用来建、删、列这个会话上的定时任务的
//! `cron_create`、`cron_delete`、`cron_list`（包 [`SCHEDULE`]），动态循环每轮定下一次的 `schedule_wakeup`（包 [`LOOP`]）。两个包
//! 能分开关：预设关了哪个，那几件就不在工具面上。
//!
//! 工具只拿 [`miyu_tool::SchedulePort`]：唤醒表在协议端点（施工 J-1），端点造端口交进会话。给模型看的字在资源目录的
//! `software/schedule/`、`software/loop/` 下（进登记簿），给人看的在各自的 `human/`。`/loop` 是核心的斜杠命令，在端点里。

mod cron;
mod texts;
mod wakeup;

use std::path::Path;
use std::sync::Arc;

use miyu_tool::Tool;
use miyu_tool::load::LoadError;

/// 定时任务这个软件包的编号：资源目录里的名字，也是预设里开关它的那个键。
pub const SCHEDULE: &str = "schedule";

/// 循环这个软件包的编号：同上。
pub const LOOP: &str = "loop";

/// 定时任务这个包的三件。字从资源目录 `resources` 读。
///
/// # Errors
///
/// 哪一份字读不出来、写法不对。
pub fn schedule_tools(resources: &Path) -> Result<Vec<Arc<dyn Tool>>, LoadError> {
    cron::tools(resources)
}

/// 循环这个包的工具：只有 `schedule_wakeup`。字从资源目录 `resources` 读。
///
/// # Errors
///
/// 哪一份字读不出来、写法不对。
pub fn loop_tools(resources: &Path) -> Result<Vec<Arc<dyn Tool>>, LoadError> {
    Ok(vec![Arc::new(wakeup::Wakeup::load(resources)?)])
}
