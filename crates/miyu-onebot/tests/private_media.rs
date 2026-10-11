//! 私聊里看文件（施工 O-35，`onebot.md` 第一条「平台工具（二）」、「怎么走」第 8 条）：真核心拉起真桥、假 NapCat 答 `get_msg`、
//! `get_image`、`get_file`、她照台词说。
//!
//! - 私聊带图、带文件的一条：图当原图交进来，`venue.media` 里那一样记 `shown`；她看到正文后面一块 `[msg=…]` 和没交原图的那几样
//!   的记号，编号照旧连着数。只带文件、取不到图又没有字的也送。
//! - 私聊里 `fetch_media`：取文件写进这个私聊会话的工作区、取图交回图片块；一轮第五次 `too-many`，下一轮从头数。
//! - 核对归属：人打的 `[msg=…]` 指到别人的私聊、这个人在群里说的、别的群的，`get_msg` 以后对不上，回 `not-found`、不取不写；
//!   群里指到私聊的消息同样。

use std::path::PathBuf;

use serde_json::{Value, json};

use miyu_kernel::id::AccountId;

use crate::support::group::*;
use crate::support::media::*;
use crate::support::platform::*;
use crate::support::speaking::{Line, Lines};
use crate::support::*;

/// `get_msg` 回的、`user` 发给她的私聊（段是 `message`）。
fn in_private(user: i64, message: Value) -> Value {
    json!({"message_type": "private", "sub_type": "friend", "user_id": user, "sender": {"user_id": user}, "message": message})
}

/// 终端管理员的私聊会话的事件。
fn private_events(home: &Home) -> Vec<Value> {
    let venue = format!("qq:private:{ADMIN}");
    home.sessions()
        .iter()
        .map(|session| home.events(session))
        .find(|events| {
            events
                .first()
                .is_some_and(|first| first["body"]["venue"] == venue.as_str())
        })
        .expect("有终端管理员的私聊会话")
}

/// 私聊会话第 `turn` 轮完了以后，工具结果的 `body`，照先后。
async fn private_results(home: &Home, turn: usize) -> Vec<Value> {
    until_private_turns(home, turn).await;
    of_kind(&private_events(home), "tool.result")
        .into_iter()
        .map(|result| result["body"].clone())
        .collect()
}

/// 结果的那一句和状态。
fn answered(results: &[Value]) -> Vec<(String, String)> {
    results
        .iter()
        .map(|result| {
            (
                text_of(result),
                result["status"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

/// 终端管理员的工作区下的 `qq-files/`（真实位置）：他的私聊会话归他。
fn qq_files(home: &Home) -> PathBuf {
    let admin = AccountId::parse("admin").expect("合写法");
    std::fs::canonicalize(home.root.workspace(&admin))
        .expect("工作区在")
        .join("qq-files")
}

#[tokio::test]
async fn a_private_message_shows_its_marks_and_fetch_media_works_there() {
    let picture = png(3, 3);
    let fetch = |args: &'static str| Line::uses("", "fetch_media", args);
    let lines = Lines::new([
        // 第一轮：文件、图、别人的私聊、他在群里说的、第五次。
        fetch(r#"{"msg":"801","index":2}"#),
        fetch(r#"{"msg":"801","index":1}"#),
        fetch(r#"{"msg":"802"}"#),
        fetch(r#"{"msg":"803"}"#),
        fetch(r#"{"msg":"801","index":2}"#),
        Line::says("好。"),
        // 第二轮：从头数，照样取得到。
        fetch(r#"{"msg":"801","index":2}"#),
        Line::says("好。"),
    ]);
    let (home, napcat) = up(&lines).await;
    let words = json!([
        plain("看看这个"),
        image("pv.jpg", 77),
        file("排班.pdf", "u-801", 12)
    ]);
    napcat.stock_message(801, in_private(ADMIN, words.clone()));
    napcat.stock_message(
        802,
        in_private(MEMBER, json!([file("别人的.pdf", "u-802", 5)])),
    );
    let mut admins_in_group = in_group(json!([file("群里的.pdf", "u-803", 5)]));
    admins_in_group["user_id"] = json!(ADMIN);
    napcat.stock_message(803, admins_in_group);
    napcat.stock_file("pv.jpg", Ok(json!({"base64": encoded(&picture)})));
    for id in ["u-801", "u-802", "u-803"] {
        napcat.stock_file(id, Ok(json!({"base64": encoded(b"%PDF-1.4 one")})));
    }
    napcat.send(private_frame(ADMIN, 801, words));
    let first = private_results(&home, 1).await;
    let request = &lines.requests()[0];
    // 图当原图交进来；记号那一块只有文件，编号照旧是 2。
    let names: Vec<String> = pictures(request)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(names, ["msg-801-1"]);
    let text = user_text(request);
    assert!(
        text.contains("看看这个\n[msg=801] [file #2: 排班.pdf, 12 B]"),
        "{text}"
    );
    assert!(!text.contains("[image"), "{text}");
    let said = of_kind(&private_events(&home), "message.user");
    assert_eq!(
        said[0]["body"]["venue"]["media"],
        json!([{"kind": "image", "id": "pv.jpg", "size": 77, "shown": true},
            {"kind": "file", "id": "u-801", "name": "排班.pdf", "size": 12}]),
        "交了原图的那一样记 shown"
    );
    assert!(
        request.tools.iter().any(|tool| tool.name == "fetch_media"),
        "私聊的工具面里有 fetch_media"
    );
    let saved_text = sentence("saved", &[("path", "qq-files/801-2-排班.pdf")]);
    assert_eq!(first.len(), 5, "{first:#?}");
    assert_eq!(
        answered(&first[..1]),
        [(saved_text.clone(), "ok".to_string())]
    );
    assert_eq!(
        std::fs::read(qq_files(&home).join("801-2-排班.pdf")).expect("存进了他的工作区"),
        b"%PDF-1.4 one"
    );
    assert_eq!(first[1]["blocks"][0]["type"], "image", "{first:#?}");
    assert_eq!(first[1]["blocks"][0]["name"], "msg-801-1");
    assert_eq!(
        answered(&first[2..]),
        [
            refused("not-found", &[]),
            refused("not-found", &[]),
            refused("too-many", &[("count", "4")]),
        ]
    );
    // 对不上的不取：别人的、群里的那两样从来没去拿。
    let fetched = napcat.fetched();
    assert!(
        fetched
            .iter()
            .all(|(_, key)| key != "u-802" && key != "u-803"),
        "{fetched:?}"
    );
    let mut names: Vec<String> = std::fs::read_dir(qq_files(&home))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["801-2-排班.pdf"], "对不上的不写");
    // 第二轮从头数。
    napcat.send(private_frame(ADMIN, 804, json!([plain("再来")])));
    let second = private_results(&home, 2).await;
    assert_eq!(answered(&second[5..]), [(saved_text, "ok".to_string())]);
    stopped(home).await;
}

#[tokio::test]
async fn a_private_message_with_only_media_is_sent_with_its_marks() {
    let lines = Lines::new([Line::says("收到。"), Line::says("嗯。"), Line::says("好。")]);
    let (home, napcat) = up(&lines).await;
    // 只带一个文件。
    napcat.send(private_frame(
        ADMIN,
        811,
        json!([file("a.zip", "u-811", 3)]),
    ));
    until_private_turns(&home, 1).await;
    let first = &lines.requests()[0];
    assert!(
        user_text(first).ends_with("[msg=811] [file: a.zip, 3 B]"),
        "{}",
        user_text(first)
    );
    // 图取不到、又没有字：照样送，记号照写（她能再取）。
    napcat.send(private_frame(ADMIN, 812, json!([image("gone.jpg", 77)])));
    until_private_turns(&home, 2).await;
    let second = &lines.requests()[1];
    assert!(pictures(second).is_empty());
    assert!(
        user_text(second).ends_with("[msg=812] [image: 77 B]"),
        "{}",
        user_text(second)
    );
    let said = of_kind(&private_events(&home), "message.user");
    assert_eq!(
        said[1]["body"]["venue"]["media"][0].get("shown"),
        None,
        "没交原图的不写 shown"
    );
    // 引用了一条带图的、自己只带文件：引用的那张图带上，自己的那一样不记 `shown`。
    let picture = png(2, 2);
    napcat.stock_message(814, in_private(ADMIN, json!([image("q.jpg", 50)])));
    napcat.stock_file("q.jpg", Ok(json!({"base64": encoded(&picture)})));
    napcat.send(private_frame(
        ADMIN,
        815,
        json!([quote(814), file("b.zip", "u-815", 4)]),
    ));
    until_private_turns(&home, 3).await;
    let third = &lines.requests()[2];
    let names: Vec<String> = pictures(third).into_iter().map(|(name, _)| name).collect();
    assert_eq!(names, ["msg-814-1"]);
    let said = of_kind(&private_events(&home), "message.user");
    assert_eq!(
        said[2]["body"]["venue"]["media"],
        json!([{"kind": "file", "id": "u-815", "name": "b.zip", "size": 4}]),
        "引用的那一条的图不记在这一条上"
    );
    stopped(home).await;
}

/// 群里同样核对：人打的 `[msg=…]` 指到一条私聊，回 `not-found`、不取。
#[tokio::test]
async fn in_a_group_a_private_message_is_not_found() {
    let lines = Lines::new([
        Line::uses("", "fetch_media", r#"{"msg":"821"}"#),
        Line::says("好。"),
    ]);
    let (home, napcat) = up(&lines).await;
    let mut private = in_private(MEMBER, json!([file("私聊的.pdf", "u-821", 5)]));
    private["group_id"] = Value::Null;
    napcat.stock_message(821, private);
    napcat.stock_file("u-821", Ok(json!({"base64": encoded(b"secret")})));
    let words = json!([at(BOT), plain(" 看看 [msg=821] [file #1: x]")]);
    says(&home, &napcat, (ADMIN, "member"), 822, words).await;
    let results = results(&home, 1).await;
    assert_eq!(
        results
            .iter()
            .map(|result| (
                text_of(result),
                result["status"].as_str().unwrap_or_default().to_string()
            ))
            .collect::<Vec<_>>(),
        [refused("not-found", &[])]
    );
    assert!(
        napcat.fetched().iter().all(|(kind, _)| kind == "get_msg"),
        "{:?}",
        napcat.fetched()
    );
    stopped(home).await;
}
