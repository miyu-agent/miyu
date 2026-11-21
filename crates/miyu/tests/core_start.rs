//! `miyu core start`、`miyu core status`（施工 CL-8，`internal/blueprint/core.md`「对外的样子」）：真的主程序、真的核心。

use serde_json::json;

use crate::support::{Home, count};
use miyu_ipc::Lock;

/// `miyu core start`、`status`（施工 CL-8，2026-11-18 项目主人定）：`start` 拉起真的核心、什么都不印，再敲一次不多拉；`status`
/// 在跑的说版本和数据根、退出码 0，停了以后说没在跑、退出码 3，不拉起；人停过的（`run/stopped`）`start` 照样拉得起。
#[tokio::test]
async fn core_start_brings_one_up_and_status_tells_whether_it_runs() {
    let home = Home::new();
    let cwd = home.root.path().to_path_buf();
    let run = |args: &'static [&'static str]| {
        let cwd = cwd.clone();
        async move { crate::support::cli::run(&cwd, &cwd, "zh_CN.UTF-8", args).await }
    };
    let place = home.root.path().display().to_string();

    let idle = run(&["core", "status"]).await;
    assert_eq!(idle.status.code(), Some(3), "{idle:?}");
    assert_eq!(
        crate::support::cli::stdout(&idle),
        format!("核心未运行\n数据根：{place}\n提示：运行 'miyu core start' 启动\n")
    );
    assert_eq!(count(&home.core_log(), "starting"), 0, "只看，不拉起");

    let started = run(&["core", "start"]).await;
    assert_eq!(started.status.code(), Some(0), "{started:?}");
    assert_eq!(
        (started.stdout.as_slice(), started.stderr.as_slice()),
        (&b""[..], &b""[..]),
        "什么都不印"
    );
    assert!(Lock::acquire(&home.root).is_err(), "回来时核心已经在跑");
    let again = run(&["core", "start"]).await;
    assert_eq!(again.status.code(), Some(0), "{again:?}");
    assert_eq!(count(&home.core_log(), "starting"), 1, "在跑的不再拉");

    let running = run(&["core", "status", "--format", "json"]).await;
    assert_eq!(running.status.code(), Some(0), "{running:?}");
    let said: serde_json::Value = serde_json::from_slice(&running.stdout).expect("一行 JSON");
    assert_eq!(
        said,
        json!({"running": true, "version": env!("CARGO_PKG_VERSION"), "data_root": place})
    );
    let text = run(&["core", "status"]).await;
    assert_eq!(
        crate::support::cli::stdout(&text),
        format!(
            "核心正在运行\n版本：{}\n数据根：{place}\n",
            env!("CARGO_PKG_VERSION")
        )
    );

    let stopped = run(&["core", "stop"]).await;
    assert_eq!(stopped.status.code(), Some(0), "{stopped:?}");
    assert!(
        home.root.run().join("stopped").is_file(),
        "留下了人停的记号"
    );
    let after = run(&["core", "status", "--format", "json"]).await;
    assert_eq!(after.status.code(), Some(3), "{after:?}");
    let said: serde_json::Value = serde_json::from_slice(&after.stdout).expect("一行 JSON");
    assert_eq!(
        said,
        json!({"running": false, "version": null, "data_root": place})
    );

    let back = run(&["core", "start"]).await;
    assert_eq!(back.status.code(), Some(0), "人停过的照样拉得起：{back:?}");
    assert_eq!(count(&home.core_log(), "starting"), 2);
    assert!(
        !home.root.run().join("stopped").exists(),
        "核心起来时删掉记号"
    );
    let stopped = run(&["core", "stop"]).await;
    assert_eq!(stopped.status.code(), Some(0), "{stopped:?}");
    Lock::acquire(&home.root).expect("收掉了");
}
