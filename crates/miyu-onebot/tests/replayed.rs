//! 回放校准（施工 O-37，`onebot.md` 第一条「群里怎么叫她」第 6、13 条，「施工时定的」第 79 条）：真核心照开关拉起真桥，假
//! NapCat 发群消息，她的回合照剧本说，判官那一次发到本机回环上的假服务器。判官判不了的，冲她来的照回、刚说过话的只记下；
//! 她刚说完话，只发一张图、只发一个合并转发（正文只有占位）的不算刚说过话、不问判官，同一个窗口里有字的照旧问。续聊的窗口
//! 在 `src/core/route/decide/tests.rs`（出厂参数、不用真等 20 秒）。

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};

use crate::judged::{JIE, LIN, MEMBERS, decided, until_decided};
use crate::support::group::*;
use crate::support::judge::*;
use crate::support::media::image;
use crate::support::*;

/// 不抽样的群。
const GROUP: i64 = 571;

/// 系统的场所规则：群都不抽样（结论不随序号变）。
const RULES: &str = "[[rule]]\nmatch = { kind = \"group\" }\nchatty = { probability = 0 }\n";

/// 群号是 `group` 的场所编号。
fn venue(group: i64) -> String {
    format!("qq:group:{group}")
}

/// 第 `message` 条消息的命令编号加 `/what`。
fn cause(message: i64, what: &str) -> String {
    format!("qq:{BOT}:{message}:{TIME}/{what}")
}

/// 等到群 `group` 里有 `n` 条 `kind` 的事件：交回那时的全部事件。
async fn until_count(home: &Home, group: i64, kind: &str, n: usize) -> Vec<Value> {
    until_events(&home.root, &venue(group), |events| {
        of_kind(events, kind).len() == n
    })
    .await
}

/// 终端管理员在群里 @ 她说第 `message` 条。
fn admin_calls(napcat: &Answering, message: i64) {
    napcat.send(group_frame(
        GROUP,
        ADMIN,
        message,
        json!([at(BOT), plain(" 在吗")]),
        ("终端管理员", "o"),
    ));
}

#[tokio::test]
async fn a_call_the_judge_cannot_judge_is_answered_but_a_follow_up_is_not() {
    // 每一条都读不出（问两次）；多备一份，多问了看得出来。
    let server = judge(vec![
        says("我觉得应该回"),
        says("还是应该回"),
        says("不知道"),
        says("真不知道"),
        yes("spare"),
    ])
    .await;
    let script = Script::new([Play::Says("在呢。"), Play::Says("嗯。")]);
    let (home, mut napcat, _) = started_judged(&script, &server, (RULES, ""), "", MEMBERS).await;
    // 1：小林 @ 她，判官判不了：冲她来的照回。
    napcat.send(group_frame(
        GROUP,
        LIN,
        1,
        json!([at(BOT), plain(" 在吗")]),
        ("小林", "lin"),
    ));
    assert_eq!(napcat.group_reply(GROUP).await, "在呢。");
    // 刚说过话照送达算：等 `venue.delivered` 记下了再接着说。
    until_count(&home, GROUP, "venue.delivered", 1).await;
    // 2：阿杰接一句（没 @ 她）：刚说过话，判官判不了，只记下。
    napcat.send(group_frame(
        GROUP,
        JIE,
        2,
        json!([plain("是吗")]),
        ("阿杰", "jie"),
    ));
    let events = until_decided(&home, GROUP, 2).await;
    // 3：终端管理员 @ 她：这一轮是第二轮，前面没有多开的。
    admin_calls(&napcat, 3);
    assert_eq!(napcat.group_reply(GROUP).await, "嗯。");
    let after = until_count(&home, GROUP, "turn.ended", 2).await;

    let called = decided(&events, 1).expect("记了判断");
    assert_eq!(
        called["conditions"],
        json!([{"kind": "direct", "bonus": 0.3}])
    );
    assert_eq!(called["route"], "judge", "{called}");
    assert_eq!(
        called["judge"],
        json!({"mode": "reply", "tries": 2, "millis": called["judge"]["millis"], "model": JUDGE_MODEL,
            "unjudged": "unreadable", "detail": "no_object"}),
        "{called}"
    );
    assert!(called.get("score").is_none(), "判不了没有分：{called}");
    assert_eq!(called["outcome"], "reply", "冲她来的判不了照回：{called}");

    let followed = decided(&events, 2).expect("记了判断");
    assert_eq!(
        followed["conditions"],
        json!([{"kind": "after_speaking", "bonus": 0.1}])
    );
    assert_eq!(followed["judge"]["unjudged"], "unreadable", "{followed}");
    assert_eq!(
        followed["outcome"], "record",
        "别的判不了照不回算：{followed}"
    );

    let started = of_kind(&after, "turn.started");
    let causes: Vec<&Value> = started.iter().map(|turn| &turn["cause"]).collect();
    assert_eq!(
        causes,
        [&json!(cause(1, "respond")), &json!(cause(3, "respond"))],
        "{started:#?}"
    );
    assert_eq!(server.received().len(), 4, "每条问两次，没多问");
    assert!(run_log(&home.root).contains("not judged"));
    assert!(napcat.pending().is_none(), "别的什么都不发");
    stopped(home).await;
}

#[tokio::test]
async fn textless_messages_after_she_speaks_are_only_recorded() {
    // 判官只该被问一次（有字的那一句）；多备两份，多问了看得出来。
    let server = judge(vec![no("just chatting"), yes("spare"), yes("spare")]).await;
    let script = Script::new([Play::Says("在。"), Play::Says("嗯。")]);
    let (home, mut napcat, _) = started_judged(&script, &server, (RULES, ""), "", MEMBERS).await;
    admin_calls(&napcat, 1);
    assert_eq!(napcat.group_reply(GROUP).await, "在。");
    until_count(&home, GROUP, "venue.delivered", 1).await;
    // 2：小林只发一张图。3：小林只发一个合并转发（取不到，正文只有占位）。都在她说完的 30 秒里。
    napcat.send(group_frame(
        GROUP,
        LIN,
        2,
        json!([image("p.jpg", 2048)]),
        ("小林", "lin"),
    ));
    until_decided(&home, GROUP, 2).await;
    napcat.send(group_frame(
        GROUP,
        LIN,
        3,
        json!([{"type": "forward", "data": {"id": "fw-1"}}]),
        ("小林", "lin"),
    ));
    until_decided(&home, GROUP, 3).await;
    // 4：阿杰说一句有字的：窗口还开着，照旧刚说过话、问判官。
    napcat.send(group_frame(
        GROUP,
        JIE,
        4,
        json!([plain("这是什么")]),
        ("阿杰", "jie"),
    ));
    let events = until_decided(&home, GROUP, 4).await;
    admin_calls(&napcat, 5);
    assert_eq!(napcat.group_reply(GROUP).await, "嗯。");
    until_count(&home, GROUP, "turn.ended", 2).await;

    for message in [2, 3] {
        let body = decided(&events, message).expect("记了判断");
        assert_eq!(
            (&body["conditions"], &body["route"], &body["outcome"]),
            (&json!([]), &json!("record"), &json!("record")),
            "没有字的不算刚说过话：{body}"
        );
        assert!(body.get("judge").is_none(), "{body}");
    }
    let worded = decided(&events, 4).expect("记了判断");
    assert_eq!(
        worded["conditions"],
        json!([{"kind": "after_speaking", "bonus": 0.1}])
    );
    assert_eq!(
        worded["judge"]["answer"]["reason"], "just chatting",
        "{worded}"
    );
    assert_eq!(server.received().len(), 1, "判官只问了有字的那一句");
    // 近况里照常写：图的那一条记下了带的东西，转发的那一条正文是占位。
    let said = said(&events);
    assert_eq!(
        said[1]["body"]["venue"]["media"][0]["kind"], "image",
        "{said:#?}"
    );
    assert_eq!(words(&said[2]), "[forward]", "{said:#?}");
    stopped(home).await;
}
