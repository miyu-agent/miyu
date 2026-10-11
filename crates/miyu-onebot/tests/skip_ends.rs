//! 不说话就结束这一轮（施工 O-38，`onebot.md` 第一条「提供者和不说话」第 3 条，施工时定的第 238 条；核心那一半是 4-14）：真核心
//! 拉起真桥、假 NapCat、她照台词说（`support/speaking.rs`）。桥答 `skip_reply` 带 `end_turn`，这一步齐了核心不再请求：这一轮里又
//! 叫她的，核心接着开下一轮，她在新一轮里的回答照常发；同一步别的工具照常跑完。

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::oneshot;

use crate::support::group::*;
use crate::support::platform::next_platform;
use crate::support::speaking::{Line, Lines, SKIP_REPLY};
use crate::support::*;

/// 不抽样的群：别人的话只记下，终端管理员 @ 她开一轮。
const GROUP: i64 = 778;

/// 系统的场所规则：群都不抽样。
const RULES: &str = "[[rule]]\nmatch = { kind = \"group\" }\nchatty = { probability = 0 }\n";

/// 假 NapCat 认得的群成员：她自己。
const MEMBERS: &[Member] = &[(BOT, "米尤", "miyu")];

/// 群号是 [`GROUP`] 的场所编号。
fn venue() -> String {
    format!("qq:group:{GROUP}")
}

/// 终端管理员在群里 @ 她：第 `message` 条，说 `text`。
fn admin_calls(napcat: &Answering, message: i64, text: &str) {
    let words = json!([at(BOT), plain(&format!(" {text}"))]);
    napcat.send(group_frame(
        GROUP,
        ADMIN,
        message,
        words,
        ("终端管理员", "o"),
    ));
}

/// 等到群里有 `n` 条 `kind` 的事件：交回那时的全部事件。
async fn until_count(home: &Home, kind: &str, n: usize) -> Vec<Value> {
    until_events(&home.root, &venue(), |events| {
        of_kind(events, kind).len() == n
    })
    .await
}

/// 一段字。
fn words(text: &str) -> Value {
    json!({"type": "text", "data": {"text": text}})
}

#[tokio::test]
async fn a_call_joined_into_a_skipped_turn_is_answered_in_the_next_turn() {
    let (release, held) = oneshot::channel();
    let lines = Lines::new([
        Line::skips("不是叫我吧。").released_by(held),
        Line::says("在，看到了。"),
    ]);
    let (home, mut napcat, _) = started_by(Arc::new(lines.clone()), RULES, MEMBERS).await;
    admin_calls(&napcat, 1, "在吗");
    until_count(&home, "turn.started", 1).await;
    // 她那次请求还压着，终端管理员又 @ 她：并进这一轮。
    admin_calls(&napcat, 2, "问你呢");
    let events = until_count(&home, "turn.joined", 1).await;
    let joined = of_kind(&events, "turn.joined")[0]["seq"].clone();
    // 放行：她调 `skip_reply`，这一轮结束；核心由那条 `turn.joined` 接着开一轮，她的回答发到群里。
    release.send(()).expect("她在等");
    assert_eq!(napcat.group_message(GROUP).await, [words("在，看到了。")]);
    let events = until_count(&home, "turn.ended", 2).await;
    let ended = of_kind(&events, "turn.ended");
    assert_eq!(ended[0]["body"]["reason"], "completed", "{events:#?}");
    assert_eq!(
        ended[0]["by"]["kind"], "tool",
        "是那一次调用结束的：{events:#?}"
    );
    let started = of_kind(&events, "turn.started");
    assert_eq!(started[1]["body"]["trigger"], joined, "{events:#?}");
    assert_ne!(started[1]["turn"], started[0]["turn"], "回合编号是新的");
    assert_eq!(lines.requests().len(), 2, "跳过的那一轮只请求一次");
    assert!(napcat.pending().is_none(), "跳过的那一句没发");
    stopped(home).await;
}

#[tokio::test]
async fn other_calls_of_the_step_still_run() {
    // 多排一句：照旧接着请求的话拿得到，不卡在替身里。
    let lines = Lines::new([Line::skips("").and("poke", "{}"), Line::says("不该说。")]);
    let (home, mut napcat, _) = started_by(Arc::new(lines.clone()), RULES, MEMBERS).await;
    admin_calls(&napcat, 1, "戳我一下");
    // 戳照常到 NapCat：没 @ 别人，戳叫她的人。
    let action = next_platform(&mut napcat).await;
    assert_eq!(action["action"], "group_poke", "{action}");
    assert_eq!(
        action["params"],
        json!({"group_id": GROUP, "user_id": ADMIN})
    );
    let events = until_count(&home, "turn.ended", 1).await;
    let results = of_kind(&events, "tool.result");
    let assistant = &of_kind(&events, "message.assistant")[0];
    let name = |result: &Value| {
        let blocks = assistant["body"]["blocks"].as_array().expect("有块");
        let call = blocks
            .iter()
            .find(|block| block["call_id"] == result["body"]["call_id"])
            .expect("结果对得上调用");
        call["name"].as_str().expect("有名字").to_string()
    };
    let mut seen: Vec<(String, Value, Value)> = results
        .iter()
        .map(|one| {
            let body = &one["body"];
            (name(one), body["status"].clone(), body["end_turn"].clone())
        })
        .collect();
    seen.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        seen,
        [
            ("poke".to_string(), json!("ok"), Value::Null),
            (SKIP_REPLY.to_string(), json!("ok"), json!(true)),
        ],
        "{events:#?}"
    );
    let ended = &of_kind(&events, "turn.ended")[0];
    assert_eq!(ended["body"]["reason"], "completed", "{ended}");
    let last = results
        .iter()
        .filter_map(|one| one["seq"].as_u64())
        .max()
        .expect("有结果");
    assert!(
        ended["seq"].as_u64() > Some(last),
        "两件都到了才结束：{events:#?}"
    );
    assert_eq!(lines.requests().len(), 1, "只请求一次");
    stopped(home).await;
}
