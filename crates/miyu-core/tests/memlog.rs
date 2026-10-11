//! 核心自己记内存账（施工 V-4，`docs/blueprint/heap.md`「自己记账」）：起来就记一行，到点再记，涨得快的当场多记一行；
//! 量不出来的平台不起、不写文件。

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use miyu_core::memlog::{self, FILE, Timing};
use miyu_session::testkit::Script;

use crate::support::Home;

/// 记下的几行。
fn lines(home: &Home) -> Vec<Value> {
    std::fs::read_to_string(home.root.state().join("logs").join(FILE))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).expect("一行一个 JSON"))
        .collect()
}

/// 等到记下的行里有 `why` 是 `wanted` 的，交回全部的行。
async fn until_why(home: &Home, wanted: &str) -> Vec<Value> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let lines = lines(home);
        if lines.iter().any(|line| line["why"] == wanted) {
            return lines;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {wanted}：{lines:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn the_core_samples_itself_at_start_and_on_the_tick() {
    let home = Home::new();
    let core = home.core(Arc::new(Script::new([])), "t");
    let timing = Timing {
        every: Duration::from_millis(200),
        peek: Duration::from_millis(20),
        grew: u64::MAX,
    };
    let task = memlog::spawn(&core, &home.root.state().join("logs"), timing);
    if !cfg!(target_os = "linux") {
        assert!(task.is_none(), "量不出来的平台不起");
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(lines(&home).is_empty());
        return;
    }
    let lines = until_why(&home, "tick").await;
    assert_eq!(lines[0]["why"], "start", "{lines:?}");
    let first = &lines[0];
    assert!(first["rss"].as_u64().is_some_and(|rss| rss > 0), "{first}");
    assert!(first["pss"].as_u64().is_some_and(|pss| pss > 0), "{first}");
    assert_eq!(first["sessions"], 0);
    assert!(first["at"].as_str().is_some_and(|at| at.ends_with('Z')));
    assert_eq!(
        first.get("heap").is_some(),
        cfg!(target_env = "gnu"),
        "glibc 上带堆的统计：{first}"
    );
    if let Some(task) = task {
        task.abort();
    }
}

#[tokio::test]
async fn growing_fast_takes_one_more_sample() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let home = Home::new();
    let core = home.core(Arc::new(Script::new([])), "t");
    let timing = Timing {
        every: Duration::from_secs(3600),
        peek: Duration::from_millis(20),
        grew: 16 * 1024 * 1024,
    };
    let task = memlog::spawn(&core, &home.root.state().join("logs"), timing);
    until_why(&home, "start").await;
    // 64 MiB、每一页都写过：真的进了物理内存。
    let held = vec![1u8; 64 * 1024 * 1024];
    let lines = until_why(&home, "grew").await;
    assert!(
        lines.iter().all(|line| line["why"] != "tick"),
        "还没到点：{lines:?}"
    );
    std::hint::black_box(&held);
    if let Some(task) = task {
        task.abort();
    }
}
