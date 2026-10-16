//! 群里的支线（施工 O-36，`onebot.md` 第一条「群里怎么叫她」第 1、2、6、7、9 条；`session/fork.md`）：真核心拉起真桥、假
//! NapCat、她照台词说（`support/speaking.rs`）。主线在回 A 的时候 B @ 她：分叉一条支线回 B，支线第一次请求的前缀和主线上一次
//! 请求逐字节一样，开轮带一块「另一条线正在回」；A 再催并进主线，B 再催抄进支线并进，支线满了 C 也并进主线；两条线的话各引用
//! 各回的那一条，`venue.delivered` 都记在主线、`line` 照线写；支线跑完一轮封存。主线开轮时支线还在回的带上那一块，支线封存以后
//! 主线下一轮的近况里有它说的话。支线里调平台工具、`fetch_media` 照支线自己的触发、找得到发到哪；支线开的回合算进限流；支线的话
//! 发不出去，退信记在她看得到的那一处。

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::oneshot;

use miyu_kernel::request::Request;

use crate::support::group::*;
use crate::support::speaking::{Line, Lines};
use crate::support::*;

/// 群号。
const GROUP: i64 = 3601;

/// 群里的几个人。
const A: i64 = 20011;
const B: i64 = 20012;
const C: i64 = 20013;
const Z: i64 = 20019;

/// 假 NapCat 认得的群成员。
const MEMBERS: &[Member] = &[
    (BOT, "米尤", "miyu"),
    (A, "阿甲", "a"),
    (B, "阿乙", "b"),
    (C, "阿丙", "c"),
    (Z, "阿庚", "z"),
];

/// 系统的场所规则：叫她才回（不过判官），总引用她回的那一条，限流照 `rate`；支线照出厂的 1 条。
fn rules(rate: &str) -> String {
    format!(
        "[[rule]]\nmatch = {{ kind = \"group\" }}\ndiscipline = \"when-called\"\nrate = \"{rate}\"\noutbound = {{ quote_after = 0 }}\n"
    )
}

/// 场所编号。
fn venue() -> String {
    format!("qq:group:{GROUP}")
}

/// `user` @ 她说第 `message` 条，后面再 @ 几个人（`more`）。
fn calls(napcat: &Answering, user: i64, message: i64, text: &str, more: &[i64]) {
    let mut words = vec![at(BOT), plain(&format!(" {text}"))];
    words.extend(more.iter().map(|other| at(*other)));
    let (_, card, nickname) = MEMBERS
        .iter()
        .find(|(id, _, _)| *id == user)
        .expect("认得的成员");
    napcat.send(group_frame(
        GROUP,
        user,
        message,
        json!(words),
        (card, nickname),
    ));
}

/// 出厂的「另一条线正在回」那一块：别的线在回第 `message` 条、这条线回第 `aim` 条，去掉行尾。
fn another_line(message: i64, aim: i64) -> String {
    let path = resources().join("packages/onebot/data/facts/another-line.txt");
    std::fs::read_to_string(path)
        .expect("读得出")
        .replace("{msg}", &message.to_string())
        .replace("{aim}", &aim.to_string())
        .trim_end()
        .to_string()
}

/// 一次请求最后一条消息写成的 JSON。
fn last(request: &Request) -> String {
    serde_json::to_string(request.messages.last().expect("有消息")).expect("写得成")
}

/// 两次请求的 system、工具面一样，消息除了最后一条（这一轮新来的那一条）都逐字节一样。
fn same_prefix(a: &Request, b: &Request) {
    assert_eq!(a.system, b.system, "system 一样");
    assert_eq!(a.tools, b.tools, "工具面一样");
    assert_eq!(a.messages.len(), b.messages.len(), "{a:#?}\n---\n{b:#?}");
    let n = a.messages.len() - 1;
    assert!(n > 0, "要有前缀可比");
    assert_eq!(
        serde_json::to_string(&a.messages[..n]).expect("写得成"),
        serde_json::to_string(&b.messages[..n]).expect("写得成"),
        "前面那一段逐字节一样"
    );
}

/// 等到她收到第 `n` 次主请求。
async fn until_requests(lines: &Lines, n: usize) {
    within("她收到请求", async {
        while lines.requests().len() < n {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await;
}

/// 等到这个群的支线合 `wanted`：交回那时的支线（编号和事件）。
async fn until_lanes(
    home: &Home,
    wanted: impl Fn(&[(String, Vec<Value>)]) -> bool,
) -> Vec<(String, Vec<Value>)> {
    within("等支线", async {
        loop {
            let lanes = lane_sessions(&home.root, &venue());
            if wanted(&lanes) {
                return lanes;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
}

/// 等到主线有 `n` 条 `kind` 的事件：交回那时的全部事件。
async fn until_main(home: &Home, kind: &str, n: usize) -> Vec<Value> {
    until_events(&home.root, &venue(), |events| {
        of_kind(events, kind).len() >= n
    })
    .await
}

/// 她发进群里的下一句：（引用的平台编号，字）。没引用的是空的。
async fn next_said(napcat: &mut Answering) -> (Option<String>, String) {
    let message = napcat.group_message(GROUP).await;
    let quoted = message
        .iter()
        .find(|segment| segment["type"] == "reply")
        .and_then(|segment| segment["data"]["id"].as_str())
        .map(str::to_string);
    let text = message.last().expect("有段")["data"]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    (quoted, text)
}

/// 主线里平台编号是 `message` 的那一条人说的话的序号。
fn seq_of(events: &[Value], message: i64) -> u64 {
    said(events)
        .iter()
        .find(|said| said["body"]["venue"]["msg"] == message.to_string().as_str())
        .and_then(|said| said["seq"].as_u64())
        .unwrap_or_else(|| panic!("没有第 {message} 条：{events:#?}"))
}

#[tokio::test]
async fn a_second_caller_gets_a_lane_of_his_own() {
    let (release_a, hold_a) = oneshot::channel();
    let (release_b, hold_b) = oneshot::channel();
    let lines = Lines::new([
        Line::says("在。"),
        Line::says("回A。").released_by(hold_a),
        Line::says("回B。").released_by(hold_b),
        Line::says("回A和C。"),
        Line::says("再回B。"),
    ]);
    let (home, mut napcat, _) =
        started_by(Arc::new(lines.clone()), &rules("100/1h"), MEMBERS).await;
    calls(&napcat, A, 101, "在吗", &[]);
    assert_eq!(next_said(&mut napcat).await.1, "在。");
    until_main(&home, "turn.ended", 1).await;
    // 主线回 A，压着；B @ 她：分叉一条支线回他。
    calls(&napcat, A, 102, "明天几点开会", &[]);
    until_requests(&lines, 2).await;
    calls(&napcat, B, 201, "帮我想个标题", &[]);
    until_requests(&lines, 3).await;
    let requests = lines.requests();
    same_prefix(&requests[1], &requests[2]);
    assert!(
        last(&requests[2]).contains(&another_line(102, 201)),
        "支线开轮带主线在回的那一条：{}",
        last(&requests[2])
    );
    assert!(
        !last(&requests[1]).contains("another-line"),
        "主线开轮时没有支线"
    );
    let lanes = until_lanes(&home, |lanes| lanes.len() == 1).await;
    let (lane, events) = &lanes[0];
    let main = venue_session(&home.root, &venue()).expect("有主线").0;
    assert_eq!(events[0]["body"]["forked"]["from"], main.as_str());
    assert_eq!(events[0]["body"]["forked"]["seal_after"], 1);
    // A 再催并进主线；B 再催抄进支线并进；支线满了，C 也并进主线。
    calls(&napcat, A, 103, "？", &[]);
    until_main(&home, "turn.joined", 1).await;
    calls(&napcat, B, 202, "？？", &[]);
    let copied = format!("qq:{BOT}:202:{TIME}/branch");
    let lanes = until_lanes(&home, |lanes| {
        of_kind(&lanes[0].1, "turn.joined").len() == 1
    })
    .await;
    let copy = said(&lanes[0].1)
        .into_iter()
        .find(|said| said["cause"] == copied.as_str())
        .unwrap_or_else(|| panic!("抄进了支线：{:#?}", lanes[0].1));
    assert_eq!(copy["body"]["venue"]["ambient"], true);
    assert_eq!(copy["by"]["id"], format!("qq:{B}"));
    let joined = &of_kind(&lanes[0].1, "turn.joined")[0];
    assert_eq!(joined["body"]["triggers"], json!([copy["seq"]]));
    calls(&napcat, C, 301, "我也问一下", &[]);
    let events = until_main(&home, "turn.joined", 2).await;
    let joins = of_kind(&events, "turn.joined");
    assert_eq!(joins[0]["body"]["triggers"], json!([seq_of(&events, 103)]));
    assert_eq!(joins[1]["body"]["triggers"], json!([seq_of(&events, 301)]));
    // 主线说完，再放支线：各引用各在回的那一条。
    release_a.send(()).expect("还在等");
    assert_eq!(
        next_said(&mut napcat).await,
        (Some("301".into()), "回A。".into())
    );
    assert_eq!(next_said(&mut napcat).await.1, "回A和C。");
    release_b.send(()).expect("还在等");
    assert_eq!(
        next_said(&mut napcat).await,
        (Some("202".into()), "回B。".into())
    );
    assert_eq!(next_said(&mut napcat).await.1, "再回B。");
    let lanes = until_lanes(&home, |lanes| {
        !of_kind(&lanes[0].1, "session.sealed").is_empty()
    })
    .await;
    let started = of_kind(&lanes[0].1, "turn.started")[0]["seq"].clone();
    let events = until_main(&home, "venue.delivered", 5).await;
    let delivered = of_kind(&events, "venue.delivered");
    let lane_said: Vec<_> = delivered
        .iter()
        .filter(|one| one["body"]["line"] == lane.as_str())
        .collect();
    assert_eq!(lane_said.len(), 2, "{delivered:#?}");
    assert_eq!(lane_said[0]["body"]["turn"], started);
    assert_eq!(lane_said[0]["body"]["to"], json!([format!("qq:{B}")]));
    assert!(
        of_kind(&lanes[0].1, "venue.delivered").is_empty(),
        "记在主线"
    );
    stopped(home).await;
}

#[tokio::test]
async fn the_main_line_names_what_a_lane_answers_and_later_sees_what_it_said() {
    let (release_a, hold_a) = oneshot::channel();
    let (release_b, hold_b) = oneshot::channel();
    let lines = Lines::new([
        Line::says("回A。").released_by(hold_a),
        Line::says("回B。").released_by(hold_b),
        Line::says("回C。"),
        Line::says("又回B。"),
    ]);
    let (home, mut napcat, _) =
        started_by(Arc::new(lines.clone()), &rules("100/1h"), MEMBERS).await;
    calls(&napcat, A, 101, "在吗", &[]);
    until_requests(&lines, 1).await;
    calls(&napcat, B, 201, "帮我想个标题", &[]);
    until_requests(&lines, 2).await;
    release_a.send(()).expect("还在等");
    assert_eq!(next_said(&mut napcat).await.1, "回A。");
    until_main(&home, "turn.ended", 1).await;
    // 主线闲了、支线还在回 B：C 开主线，带上 B 那一条。
    calls(&napcat, C, 301, "在吗", &[]);
    until_requests(&lines, 3).await;
    assert!(
        last(&lines.requests()[2]).contains(&another_line(201, 301)),
        "{}",
        last(&lines.requests()[2])
    );
    assert_eq!(next_said(&mut napcat).await.1, "回C。");
    release_b.send(()).expect("还在等");
    assert_eq!(next_said(&mut napcat).await.1, "回B。");
    until_lanes(&home, |lanes| {
        !of_kind(&lanes[0].1, "session.sealed").is_empty()
    })
    .await;
    until_main(&home, "venue.delivered", 3).await;
    // 支线封存了：B 再叫她，主线开一轮，近况里有支线说的那句、不再带那一块。
    calls(&napcat, B, 202, "还在吗", &[]);
    assert_eq!(next_said(&mut napcat).await.1, "又回B。");
    let request = last(&lines.requests()[3]);
    assert!(
        request.contains("[you]") && request.contains("回B。"),
        "{request}"
    );
    assert!(!request.contains("another-line"), "{request}");
    let events = until_main(&home, "turn.started", 3).await;
    let started = of_kind(&events, "turn.started");
    assert_eq!(
        started[2]["body"]["triggers"],
        json!([seq_of(&events, 202)])
    );
    assert_eq!(lane_sessions(&home.root, &venue()).len(), 1, "没再分叉");
    stopped(home).await;
}

#[tokio::test]
async fn tools_on_a_lane_look_at_its_own_trigger() {
    let (release_a, hold_a) = oneshot::channel();
    let lines = Lines::new([
        Line::says("回A。").released_by(hold_a),
        Line::uses("", "poke", "{}"),
        Line::uses("", "fetch_media", r#"{"msg":"999"}"#),
        Line::says("好。"),
    ]);
    let (home, mut napcat, _) =
        started_by(Arc::new(lines.clone()), &rules("100/1h"), MEMBERS).await;
    calls(&napcat, A, 101, "在吗", &[]);
    until_requests(&lines, 1).await;
    // B @ 她、也 @ 了 Z：支线戳的是 B 那一条 @ 的人。
    calls(&napcat, B, 201, "戳他", &[Z]);
    let action = napcat.action().await;
    assert_eq!(action["action"], "group_poke", "{action}");
    assert_eq!(action["params"], json!({"group_id": GROUP, "user_id": Z}));
    let lanes = until_lanes(&home, |lanes| {
        lanes.len() == 1 && of_kind(&lanes[0].1, "tool.result").len() == 2
    })
    .await;
    let results = of_kind(&lanes[0].1, "tool.result");
    let fetched = results[1]["body"]["blocks"][0]["text"]
        .as_str()
        .unwrap_or_default();
    let not_found = std::fs::read_to_string(
        resources().join("packages/onebot/data/tool-results/not-found.txt"),
    )
    .expect("读得出");
    assert_eq!(
        fetched.trim_end(),
        not_found.trim_end(),
        "找得到发到哪：{results:#?}"
    );
    assert_eq!(next_said(&mut napcat).await.1, "好。");
    release_a.send(()).expect("还在等");
    assert_eq!(next_said(&mut napcat).await.1, "回A。");
    stopped(home).await;
}

#[tokio::test]
async fn turns_opened_on_a_lane_count_toward_the_rate() {
    let (release_a, hold_a) = oneshot::channel();
    let (release_b, hold_b) = oneshot::channel();
    let lines = Lines::new([
        Line::says("回A。").released_by(hold_a),
        Line::says("回B。").released_by(hold_b),
    ]);
    let (home, mut napcat, _) = started_by(Arc::new(lines.clone()), &rules("2/1h"), MEMBERS).await;
    calls(&napcat, A, 101, "在吗", &[]);
    until_requests(&lines, 1).await;
    calls(&napcat, B, 201, "在吗", &[]);
    until_requests(&lines, 2).await;
    // 两轮了（一条主线、一条支线）：C 再叫她，满了。
    calls(&napcat, C, 301, "在吗", &[]);
    let events = until_main(&home, "ext.onebot.chat.decided", 3).await;
    let decided = &of_kind(&events, "ext.onebot.chat.decided")[2]["body"];
    assert_eq!(decided["inbound"], "notice", "{decided}");
    assert_eq!(decided["why"], "rate_limited", "{decided}");
    release_a.send(()).expect("还在等");
    release_b.send(()).expect("还在等");
    let mut heard = Vec::new();
    for _ in 0..3 {
        heard.push(next_said(&mut napcat).await.1);
    }
    heard.sort();
    assert_eq!(heard.len(), 3, "提示一句、两条线各一句：{heard:?}");
    assert!(heard.contains(&"回A。".to_string()) && heard.contains(&"回B。".to_string()));
    stopped(home).await;
}

#[tokio::test]
async fn what_a_lane_could_not_send_is_noted_where_she_will_see_it() {
    let (release_a, hold_a) = oneshot::channel();
    let lines = Lines::new([Line::says("回A。").released_by(hold_a), Line::says("回B。")]);
    let (home, napcat, _) = started_tuned(
        Arc::new(lines.clone()),
        &rules("100/1h"),
        &Value::Null,
        |napcat| napcat.refusing(MEMBERS),
    )
    .await;
    calls(&napcat, A, 101, "在吗", &[]);
    until_requests(&lines, 1).await;
    calls(&napcat, B, 201, "在吗", &[]);
    // 支线那句发不出去：退信记在她看得到的地方。支线一般说完就封存了，记在主线；还没封存的记在支线，只记一处。
    let noted = |events: &[Value]| {
        of_kind(events, "context.injected")
            .into_iter()
            .filter(|one| {
                one["body"]["kind"] == "undelivered"
                    && one["body"]["text"]
                        .as_str()
                        .is_some_and(|text| text.contains("回B"))
            })
            .count()
    };
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let main = venue_session(&home.root, &venue()).map(|(_, events)| noted(&events));
        let lane = lane_sessions(&home.root, &venue())
            .first()
            .map(|(_, events)| noted(events));
        let total = main.unwrap_or(0) + lane.unwrap_or(0);
        if total > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            let again = venue_session(&home.root, &venue()).map_or(0, |(_, events)| noted(&events))
                + lane_sessions(&home.root, &venue())
                    .first()
                    .map_or(0, |(_, events)| noted(events));
            assert_eq!(again, 1, "只记一处\n{}", run_log(&home.root));
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "退信没记下\n{}",
            run_log(&home.root)
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    release_a.send(()).expect("还在等");
    stopped(home).await;
}
