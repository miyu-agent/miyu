//! `shell` 的监视（施工 MN-1 上，设计 56，`docs/blueprint/tools/shell.md`「后台」）：真的起进程，三个平台都跑。写了 `watch`
//! 的放到后台，结果那一句说每一行都会叫醒她、只认标准输出、跑到几分钟停；标准输出、标准错误分开读，标准输出另交一份给任务表
//! 攒批，写文件的那一份两路都有。不写的照旧一根管道，没有那一份。

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use miyu_kernel::block::Block;
use miyu_tool::{Background, Done, JobPort};

use crate::support::{Site, Taken};

/// 交回的字。
fn text(done: &Done) -> String {
    done.blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

/// 在 `work/` 里执行 `command`，`watch` 照给的，交给 `jobs`。
async fn run(site: &Site, jobs: &Arc<Taken>, command: &str, watch: bool) -> Done {
    let args = match watch {
        true => json!({ "command": command, "description": "Watch", "watch": true }),
        false => json!({ "command": command, "description": "Count", "run_in_background": true }),
    };
    let port: Arc<dyn JobPort> = Arc::clone(jobs) as Arc<dyn JobPort>;
    site.done_jobs("shell", args, Some(port), None).await
}

/// 两份都读到头，最多三十秒：写文件的那一份、标准输出的那一份（没有的是空的）。
async fn drain(command: Background) -> (String, Option<String>) {
    let Background {
        output,
        process,
        lines,
    } = command;
    let read = tokio::task::spawn_blocking(move || output.collect::<String>());
    let copied = tokio::task::spawn_blocking(move || lines.map(|lines| lines.collect::<String>()));
    let waited = tokio::task::spawn_blocking(move || process.wait().expect("等得了"));
    tokio::time::timeout(Duration::from_secs(30), async {
        let output = read.await.expect("没 panic");
        let lines = copied.await.expect("没 panic");
        waited.await.expect("没 panic");
        (output, lines)
    })
    .await
    .expect("三十秒内结束")
}

/// 这台机器上的写法：先往标准输出写两行，再往标准错误写一行。
fn both() -> &'static str {
    if cfg!(windows) {
        "Write-Output one; Write-Output two; [Console]::Error.WriteLine('err')"
    } else {
        "echo one; echo two; echo err >&2"
    }
}

#[tokio::test]
async fn watching_runs_in_the_background_and_says_how_it_wakes_her() {
    let site = Site::new();
    let jobs = Arc::new(Taken::default());
    let done = run(&site, &jobs, both(), true).await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(
        text(&done),
        "Watching j7 in the background. Each stdout line wakes you, so print only lines you must act on. stderr goes only to the output file. It stops after 30 minutes; start it again to keep watching.\n"
    );
    let (output, lines) = drain(jobs.take(0)).await;
    assert_eq!(lines.as_deref(), Some("one\ntwo\n"), "只有标准输出");
    for line in ["one\n", "two\n", "err\n"] {
        assert!(output.contains(line), "写文件的两路都有：{output:?}");
    }
}

#[tokio::test]
async fn a_plain_background_command_keeps_one_pipe() {
    let site = Site::new();
    let jobs = Arc::new(Taken::default());
    run(&site, &jobs, both(), false).await;
    let (output, lines) = drain(jobs.take(0)).await;
    assert_eq!(lines, None, "不监视的没有那一份");
    assert!(output.contains("err\n"));
}

#[tokio::test]
async fn watching_without_a_job_port_does_not_run() {
    let site = Site::new();
    let done = site
        .done_jobs(
            "shell",
            json!({ "command": both(), "description": "Watch", "watch": true }),
            None,
            None,
        )
        .await;
    assert!(done.error);
    assert!(text(&done).starts_with("Running in the background is not available here."));
}
