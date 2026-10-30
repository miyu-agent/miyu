//! `miyu ask` 等后台命令（施工 AP-4，`docs/blueprint/cli/ask.md`「等子代理」，2026-10-31 项目主人定）：在进程里起
//! 一个核心，工具是出厂的，替身模型第一轮把一条命令放到后台就结束。命令一直等着测试放的一份文件，什么时候结束由测试定：
//! 它结束了报回来、叫醒她说了一轮，`miyu ask` 才退出。以前（施工 7-9）后台命令不等，第一轮一结束就退出，叫醒的那一轮
//! 没人看着、只记下（跑分里撞到两次：她说「等它报回来再接着」就结束了这一轮，结果没写出来）。

use serde_json::json;
use tokio::sync::mpsc;

use crate::agents::home;
use crate::support::outside::Outside;
use crate::support::router::Router;
use crate::support::{Asked, Tape, ask_onto, plan};
use miyu_cli::{Output, Plan};
use miyu_session::testkit::Play;

/// 等到工作目录里有了 `go` 才退出的命令：这台机器上的写法，Unix 上是 bash、zsh 的，Windows 上是 PowerShell 的。
fn until_go() -> &'static str {
    if cfg!(windows) {
        "while (-not (Test-Path go)) { Start-Sleep -Milliseconds 100 }"
    } else {
        "while [ ! -e go ]; do sleep 0.1; done"
    }
}

/// 照 `format` 说那一句：第一轮把等 `go` 的命令放到后台、说「放到后台了。」就结束；第一轮结束以后才放 `go`，命令结束了
/// 叫醒她说「跑完了。」。交回这一次 `miyu ask` 的结果。
async fn ask_with_a_command(format: Output) -> Asked {
    let work = Outside::new();
    let args = json!({"command": until_go(), "description": "等 go", "run_in_background": true});
    let router = Router::new(
        "后台跑一下",
        [
            Play::calls(&[("shell", &args.to_string())]),
            Play::Says("放到后台了。"),
            Play::Says("跑完了。"),
        ],
    );
    let home = home(router);
    let (_press, presses) = mpsc::channel(1);
    let plan = Plan {
        cwd: work.text(),
        format,
        ..plan("后台跑一下")
    };
    let asking = ask_onto(&home.root, &plan, presses, Tape::default());
    let releasing = async {
        let main = home.oneshot().await;
        home.until_ended(&main, 1).await;
        work.file("go", "");
    };
    let (asked, ()) = tokio::join!(asking, releasing);
    asked
}

#[tokio::test]
async fn it_waits_for_the_background_command_and_prints_the_turn_it_wakes() {
    let Asked {
        code,
        out,
        err,
        screen,
    } = ask_with_a_command(Output::Text).await;
    assert_eq!(code, 0, "{err}");
    assert_eq!(out, "放到后台了。\n跑完了。\n", "叫醒的那一轮也印了");
    assert!(
        screen.ends_with(
            "\n\n放到后台了。\n· 等待 1 个后台命令回报…（按 Ctrl+C 停止等待）\n· j1「等 go」已回报\n\n跑完了。\n\
             · 输入 300 · 命中缓存 120（40%）· 输出 30\n"
        ),
        "等它报回来、叫醒的那一轮结束了才收尾：{screen}"
    );
}

/// 事件流：命令报回来了一行 `job.reported`（编号、原因、退出码），在它叫醒的那一轮前面。
#[tokio::test]
async fn jsonl_says_when_the_background_command_reports() {
    let Asked { code, out, err, .. } = ask_with_a_command(Output::Jsonl).await;
    assert_eq!(code, 0, "{err}");
    let lines = crate::jsonl::lines(&out);
    let kept: Vec<&str> = crate::jsonl::types(&lines)
        .into_iter()
        .filter(|kind| !matches!(*kind, "text" | "thinking" | "step" | "step.done"))
        .collect();
    assert_eq!(
        kept,
        [
            "session",
            "turn.started",
            "turn.ended",
            "job.reported",
            "turn.started",
            "turn.ended",
            "done"
        ]
    );
    let reported = lines
        .iter()
        .find(|line| line["type"] == "job.reported")
        .expect("有一行");
    assert_eq!(
        reported,
        &json!({"exit": 0, "job": "j1", "reason": "exited", "type": "job.reported"})
    );
}
