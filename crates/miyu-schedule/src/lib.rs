//! 定时任务、循环这两个软件包（施工 J-2，设计 `45-定时和循环.md` 第三节第 3、4 条）：她用来建、删、列这个会话上的定时任务的
//! `cron_create`、`cron_delete`、`cron_list`（包 [`SCHEDULE`]），动态循环每轮定下一次的 `schedule_wakeup`（包 [`LOOP`]）。两个包
//! 能分开关：预设关了哪个，那几件就不在工具面上。人生会话里她自己结束这段人生的 `end_life`（包 [`LIFE`]，施工 LF-4 下，设计
//! `70-推演人生.md`）也在这里：人生的醒来就是定时表里人生那一行。
//!
//! 工具只拿 [`miyu_tool::SchedulePort`]（`end_life` 拿它带着的 [`miyu_tool::LifePort`]）：唤醒表在协议端点（施工 J-1），端点造
//! 端口交进会话。给模型看的字在资源目录的 `software/schedule/`、`software/loop/`、`software/life/` 下（进登记簿），给人看的在
//! 各自的 `human/`。`/loop` 是核心的斜杠命令，在端点里。

mod cron;
mod life;
mod texts;
mod wakeup;

use std::path::Path;
use std::sync::Arc;

use miyu_tool::Tool;
use miyu_tool::load::LoadError;

/// 定时任务这个软件包的编号：预设里开关它的那个键；资源目录另是 [`SCHEDULE_SOFTWARE`]（施工 PN-2）。
pub const SCHEDULE: &str = "base-schedule";

/// 循环这个软件包的编号：同上，资源目录另是 [`LOOP_SOFTWARE`]。
pub const LOOP: &str = "base-loop";

/// 结束人生这个软件包的编号（施工 LF-4 下）：同上，资源目录另是 [`LIFE_SOFTWARE`]。清单写了 `needs = ["life"]`。
pub const LIFE: &str = "base-life";

/// 定时任务在资源目录 `resources/software/` 里的名字，也是给人看的字的键 `software/schedule/…` 的那一段（施工 PN-2）：日志里
/// 存着，和包编号分开，包改名不跟着变。
pub const SCHEDULE_SOFTWARE: &str = "schedule";

/// 循环在资源目录里的名字：同上。
pub const LOOP_SOFTWARE: &str = "loop";

/// 结束人生在资源目录里的名字：同上。
pub const LIFE_SOFTWARE: &str = "life";

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

/// 结束人生这个包的工具：只有 `end_life`（施工 LF-4 下）。字从资源目录 `resources` 读。
///
/// # Errors
///
/// 哪一份字读不出来、写法不对。
pub fn life_tools(resources: &Path) -> Result<Vec<Arc<dyn Tool>>, LoadError> {
    Ok(vec![Arc::new(life::EndLife::load(resources)?)])
}

/// 一个包：编号和它的几件。
pub type Package = (&'static str, Vec<Arc<dyn Tool>>);

/// 这个 crate 的三个包：包的编号和它的几件，照定时、循环、人生的先后（核心照装了哪个登记哪个）。
///
/// # Errors
///
/// 同 [`schedule_tools`]。
pub fn packages(resources: &Path) -> Result<Vec<Package>, LoadError> {
    Ok(vec![
        (SCHEDULE, schedule_tools(resources)?),
        (LOOP, loop_tools(resources)?),
        (LIFE, life_tools(resources)?),
    ])
}
