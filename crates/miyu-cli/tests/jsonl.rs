//! `miyu ask --format jsonl`（施工 AP-3，设计 50 第四节，`docs/blueprint/cli/ask.md`「样子：`--format jsonl`」）：在进程里起一个
//! 核心，在真的套接字上走一遍。标准输出一行一个 JSON 对象，格照名字的字母先后排，以 `session` 开头、`done` 结尾，退出码和
//! `text` 一样；标准错误上什么都不印。

use std::sync::Arc;

use serde_json::{Value, json};

use crate::support::outside::Outside;
use crate::support::{Asked, Home, plan, shipped_tools};
use miyu_cli::{Output, Plan, Target};
use miyu_session::testkit::{Play, Script};

/// 照 `plan` 用事件流说一句。
async fn streamed(home: &Home, text: &str, target: Target) -> Asked {
    let asking = Plan {
        format: Output::Jsonl,
        target,
        ..plan(text)
    };
    home.ask(&asking).await
}

/// 标准输出的每一行读成 JSON：一行一个对象，照原样再写一遍和那一行一字不差（格照字母先后排，没有多的空白）。
pub(crate) fn lines(out: &str) -> Vec<Value> {
    assert!(out.ends_with('\n'), "以换行结尾：{out:?}");
    out.lines()
        .map(|line| {
            let value: Value = serde_json::from_str(line).expect("一行是一个 JSON");
            assert!(value.is_object(), "{line}");
            assert_eq!(value.to_string(), line, "格照字母先后排");
            value
        })
        .collect()
}

/// 每一行的 `type`。
pub(crate) fn types(lines: &[Value]) -> Vec<&str> {
    lines
        .iter()
        .map(|line| line["type"].as_str().expect("有 type"))
        .collect()
}

#[tokio::test]
async fn a_turn_streams_from_session_to_done() {
    let home = Home::new(Arc::new(Script::new([Play::Thinks {
        thinking: "想一想",
        text: "你好。",
    }])));
    let Asked { code, out, err, .. } = streamed(&home, "在吗", Target::New).await;
    assert_eq!(code, 0, "{err}");
    assert_eq!(err, "", "标准错误上什么都不印");
    let lines = lines(&out);
    let mut kinds = types(&lines);
    kinds.dedup();
    assert_eq!(
        kinds,
        [
            "session",
            "turn.started",
            "thinking",
            "text",
            "turn.ended",
            "done"
        ],
        "思考、回答一段一段"
    );
    let joined = |kind: &str| -> String {
        lines
            .iter()
            .filter(|line| line["type"] == kind)
            .map(|line| line["text"].as_str().expect("有 text"))
            .collect()
    };
    assert_eq!(joined("thinking"), "想一想");
    assert_eq!(joined("text"), "你好。");
    let session = home.sessions()[0].to_string();
    assert_eq!(
        lines[0],
        json!({"new": true, "session": session, "type": "session"})
    );
    let turn = lines[1]["turn"].as_u64().expect("有回合编号");
    let n = lines.len();
    assert_eq!(
        lines[n - 2],
        json!({"reason": "completed", "text": "你好。", "turn": turn, "type": "turn.ended",
            "usage": {"cache_read": 40, "cache_write": 0, "input": 100, "output": 10}})
    );
    assert_eq!(
        lines[n - 1],
        json!({"exit": 0, "session": session, "type": "done"})
    );
}

#[tokio::test]
async fn a_continued_session_is_not_new() {
    let home = Home::new(Arc::new(Script::new([
        Play::Says("一。"),
        Play::Says("二。"),
    ])));
    let Asked { code, err, .. } = streamed(&home, "第一句", Target::New).await;
    assert_eq!(code, 0, "{err}");
    let Asked { code, out, err, .. } = streamed(&home, "第二句", Target::Continue).await;
    assert_eq!(code, 0, "{err}");
    let lines = lines(&out);
    assert_eq!(lines[0]["new"], json!(false));
    assert_eq!(lines.last().map(|line| &line["type"]), Some(&json!("done")));
}

#[tokio::test]
async fn without_a_model_the_turn_has_the_error_and_done_says_why() {
    let home = Home::new(miyu_core::models::routes(&crate::support::resources()).expect("造得出"));
    let Asked { code, out, err, .. } = streamed(&home, "在吗", Target::New).await;
    assert_eq!(code, 5, "{err}");
    let lines = lines(&out);
    let ended = lines
        .iter()
        .find(|line| line["type"] == "turn.ended")
        .expect("有一轮结束了");
    assert_eq!(ended["reason"], "error");
    assert_eq!(ended["error"]["class"], "no_model");
    let done = lines.last().expect("有最后一行");
    assert_eq!(done["type"], "done");
    assert_eq!(done["exit"], 5);
    assert_eq!(done["reason"], "no_model");
    assert_eq!(err, "miyu: 未配置模型\n提示：运行 'miyu setup' 添加模型\n");
}

/// 每一步开始、结束各一行：工具名、标题，做成了没有、结果那一句；要确认没做的最后是一句告警，退出码 4。
#[tokio::test]
async fn steps_start_and_finish_and_skipped_ones_warn() {
    // 写到工作区外面要确认（同 `steps.rs`）：放在系统的临时目录里不算外面。
    let elsewhere = Outside::new();
    let outside = elsewhere.0.join("plan.md");
    let write = json!({"file_path": outside.to_string_lossy(), "content": "x"}).to_string();
    let home = Home::with_tools(
        Arc::new(Script::new([
            Play::calls(&[("write", &write)]),
            Play::Says("没写成。"),
        ])),
        shipped_tools(),
    );
    let Asked { code, out, err, .. } = streamed(&home, "写一下", Target::New).await;
    assert_eq!(code, 4, "{err}");
    let lines = lines(&out);
    let mut kinds = types(&lines);
    kinds.dedup();
    assert_eq!(
        kinds,
        [
            "session",
            "turn.started",
            "step",
            "step.done",
            "text",
            "turn.ended",
            "warning",
            "done"
        ]
    );
    assert_eq!(lines[2]["tool"], "write");
    let title = lines[2]["title"].as_str().expect("有标题");
    assert!(
        title.starts_with("写入 ") && title.ends_with("plan.md"),
        "{title}"
    );
    assert_eq!(lines[3]["id"], lines[2]["id"]);
    assert_eq!(lines[3]["ok"], false);
    assert_eq!(lines[3]["summary"], "需要确认，无人确认");
    let n = lines.len();
    assert_eq!(
        lines[n - 2],
        json!({"hints": ["在终端界面中运行可以确认"], "message": "1 步需要确认，已跳过", "type": "warning"})
    );
    assert_eq!(lines[n - 1]["reason"], "unattended");
    assert!(!outside.exists(), "没写");
}
