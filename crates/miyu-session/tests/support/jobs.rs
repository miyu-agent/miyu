//! 后台命令的测试共用的（施工 7-12 从 `jobs.rs` 挪出来，`jobs_moved.rs` 也用）：真的基础系统、完全放开的开场、说一句等这一轮
//! 结束、日志里的回报和工具结果、等任务表空了。

use std::path::Path;
use std::time::Duration;

use miyu_kernel::event::{Body, Event, JobReported, Level, Permission, ToolResult};
use miyu_tool::Catalog;

use super::*;

/// 真的基础系统。
pub fn base_system() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
}

/// 完全放开这一级、没人能确认，在场地的 `work/` 里：执行命令不问、不进沙盒。
pub fn full(home: &Home) -> Opening {
    Opening {
        permission: Permission {
            level: Level::Full,
            read_only: false,
        },
        attended: false,
        cwd: home.scratch.0.join("work").to_string_lossy().into_owned(),
        ..Opening::default()
    }
}

/// 说一句，等这一轮结束。
pub async fn talk(handle: &miyu_session::Handle, id: &str, words: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, id, say(words)).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 日志里的 `job.reported`，和那一条事件。
pub fn reports(log: &[Event]) -> Vec<(&Event, &JobReported)> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::JobReported(reported) => Some((event, reported)),
            _ => None,
        })
        .collect()
}

/// 日志里的工具结果。
pub fn results(log: &[Event]) -> Vec<&ToolResult> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .collect()
}

/// 等任务表空了，最多十秒。
pub async fn until_idle(home: &Home) {
    within("任务表空了", async {
        while home.jobs.running() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
}
