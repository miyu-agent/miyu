//! 认指纹不占着跑异步任务的线程（施工 V-50）：Linux 上只用 Landlock、能缓存的规格，每条命令起之前要照着规格重新看一遍文件
//! 系统、和缓存的指纹比（`miyu_sandbox::command`）。`shell` 前台、后台、脚本三处都挪到阻塞线程里做，这段时间跑异步任务的线程
//! 照样跑别的任务。
//!
//! 怎么看出来：单线程的运行时（`#[tokio::test]` 默认）里，一边调工具，一边另一个任务每 1 ms 醒一次、记下最晚晚了多久。藏起来
//! 的那一条放在一个有几万个文件的目录里，认一遍指纹要几十毫秒（先在测试线程里直接调 `miyu_sandbox::command` 量出来）。认指纹
//! 占着线程的，那个任务要等它认完，晚的不少于这么久；挪走了的，只晚起进程那一点。照「最晚晚的不到认一遍的一半」判，三次里有
//! 一次就算（机器忙时偶尔整体慢一下）。助手换成 `/bin/true`：命令不必真关起来，认指纹、带上规则集、`fork` 都照真的走。
//! 这台机器的 Landlock 用不了的（规格不缓存，本来就不认指纹）不跑。

#![cfg(target_os = "linux")]

use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::json;

use miyu_sandbox::{Sandboxed, Spec};
use miyu_tool::{JobPort, Progress, Seen, Stop};

use crate::support::{Site, Taken};

/// 另一个任务多久醒一次。
const TICK: Duration = Duration::from_millis(1);

/// 认一遍指纹至少要这么久，比得出来；不到的再往目录里加文件。
const ENOUGH: Duration = Duration::from_millis(30);

/// 每次往目录里加几个文件，最多加几回。
const BATCH: usize = 20_000;
const ROUNDS: usize = 10;

/// 每一处试几次。
const TRIES: usize = 3;

/// 场地里造一个大目录，藏起来的那一条在它里面：交回规格和直接认一遍指纹要多久。Landlock 用不了的交回没有。
fn slow(site: &Site) -> Option<(Sandboxed, Duration)> {
    let big = site.0.join("big");
    std::fs::create_dir_all(big.join("secret")).expect("建得了目录");
    let spec = Spec::from_json(
        &json!({
            "write": [site.0.join("work")],
            "hidden": [big.join("secret")],
        })
        .to_string(),
    )
    .expect("读得懂");
    let sandboxed = Sandboxed {
        helper: "/bin/true".into(),
        spec,
        env: Vec::new(),
    };
    // 照 Landlock 本身判，不照 `checks_files`：它说不认的，正是要逮的。
    if miyu_sandbox::linux::available().is_err() {
        eprintln!("这台机器的 Landlock 用不了，不认指纹，不比");
        return None;
    }
    let mut made = 0;
    for _ in 0..ROUNDS {
        for n in made..made + BATCH {
            std::fs::File::create(big.join(format!("f{n}"))).expect("建得了文件");
        }
        made += BATCH;
        // 头一次建规则集，之后三次是认指纹、缓存里拿到，取最快的。
        drop(miyu_sandbox::command(&sandboxed, Path::new("/bin/true"), &[]).expect("包得成"));
        let took = (0..3)
            .map(|_| {
                let at = Instant::now();
                drop(
                    miyu_sandbox::command(&sandboxed, Path::new("/bin/true"), &[]).expect("包得成"),
                );
                at.elapsed()
            })
            .min()
            .expect("量了三次");
        if took >= ENOUGH {
            return Some((sandboxed, took));
        }
    }
    panic!("{made} 个文件认一遍指纹还不到 {ENOUGH:?}");
}

/// 跑 `work`，同时另一个任务每 [`TICK`] 醒一次：交回 `work` 的结果和那个任务最晚晚了多久。
async fn lateness<T>(work: impl Future<Output = T>) -> (T, Duration) {
    let done = Arc::new(AtomicBool::new(false));
    let ticking = Arc::clone(&done);
    let ticker = tokio::spawn(async move {
        let mut worst = Duration::ZERO;
        while !ticking.load(Ordering::Acquire) {
            let at = Instant::now();
            tokio::time::sleep(TICK).await;
            worst = worst.max(at.elapsed().saturating_sub(TICK));
        }
        worst
    });
    // 让它先跑起来、定好第一次醒。
    tokio::task::yield_now().await;
    let out = work.await;
    done.store(true, Ordering::Release);
    (out, ticker.await.expect("没 panic"))
}

/// 照 `run` 调 [`TRIES`] 次，最晚晚的有一次不到 `took` 的一半就过；过不了的说是哪一处、每次晚了多久。
async fn check<F: Future<Output = miyu_tool::Done>>(
    what: &str,
    took: Duration,
    run: impl Fn() -> F,
) {
    let mut seen = Vec::new();
    for _ in 0..TRIES {
        let (done, worst) = lateness(run()).await;
        assert!(!done.error, "{what}：{:?}", done.blocks);
        seen.push(worst);
        if worst < took / 2 {
            return;
        }
    }
    panic!(
        "{what}：认一遍指纹 {took:?}，另一个任务每次最晚晚了 {seen:?}，认指纹占着跑异步任务的线程"
    );
}

#[tokio::test]
async fn fingerprinting_does_not_hold_the_async_thread() {
    let site = Site::new();
    let Some((sandboxed, took)) = slow(&site) else {
        return;
    };
    site.file(
        "scripts/nothing.sh",
        b"#!/bin/sh\n# Description: Nothing.\n",
    );

    check("shell 前台", took, || {
        site.done_sandboxed(
            "shell",
            json!({ "command": "true", "description": "Test" }),
            sandboxed.clone(),
        )
    })
    .await;

    let jobs = Arc::new(Taken::default());
    check("shell 后台", took, || {
        let port: Arc<dyn JobPort> = jobs.clone();
        site.done_jobs(
            "shell",
            json!({ "command": "true", "description": "Test", "run_in_background": true }),
            Some(port),
            Some(sandboxed.clone()),
        )
    })
    .await;
    let started = std::mem::take(&mut *jobs.commands.lock().expect("没 panic"));
    for background in started {
        let exit = tokio::task::spawn_blocking(move || background.process.wait())
            .await
            .expect("没 panic")
            .expect("等得了");
        assert_eq!(exit, miyu_tool::Exit::Code(0));
    }

    let script = site.script_tool("scripts/nothing.sh");
    check("脚本", took, || {
        let call = miyu_tool::Call {
            sandbox: Some(Arc::new(sandboxed.clone())),
            ..site.call_for("work", json!({}), Seen::new(), Stop::default())
        };
        script.run(call, Progress::new(|_| {}))
    })
    .await;
}
