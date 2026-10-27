//! 会话列表里的「在等人」（施工 SL-1，终端第 21 条，项目主人定）：她问着人、等确认的会话，`session.list` 那一项写
//! `waiting: true`，答完了不写；命令里脚本调的工具在等确认也算（嵌套调用）。口径同视图流会话状态的 `state: waiting`。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

/// `session.list` 里会话 `session` 那一项。
async fn item(client: &mut Client, session: &str) -> Value {
    let listed = client.call("l", "session.list", json!({})).await;
    listed["result"]["sessions"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["session"] == session))
        .cloned()
        .unwrap_or_else(|| panic!("列着：{listed}"))
}

/// 等到列表里那一项的 `waiting` 是 `want`（没写当假），最多一分钟。
async fn until_waiting(client: &mut Client, session: &str, want: bool) -> Value {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let one = item(client, session).await;
        if one.get("waiting").is_some() == want {
            if want {
                assert_eq!(one["waiting"], true, "{one}");
            }
            return one;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 waiting={want}：{one}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn a_question_marks_the_session_as_waiting_until_answered() {
    let home = Home::new();
    let args = json!({"questions": [{"question": "用哪个？", "options": [{"label": "甲"}, {"label": "乙"}]}]});
    let plays = vec![
        Play::calls(&[("ask_user", &args.to_string())]),
        Play::Says("好，用甲。"),
    ];
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).expect("读得出"))
        .expect("合写法");
    let mut client = Client::connect(home.core_with_tools(&Script::new(plays), tools, TOKEN));
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &cwd).await;
    client.say("s1", &session, "hi").await;
    let waiting = until_waiting(&mut client, &session, true).await;
    assert_eq!(waiting["busy"], true, "等人的时候回合还没完：{waiting}");
    let call = until_logged(&home, &session, "question.asked").await["body"]["call_id"].clone();
    let params = json!({"session": session, "call": call, "answers": [{"picked": ["甲"]}]});
    client.call("a", "session.answer", params).await;
    home.until_turns(&session, 1).await;
    until_waiting(&mut client, &session, false).await;
}

#[tokio::test]
async fn a_nested_approval_counts_too() {
    let home = Home::new();
    let folder = home.work.join("kit");
    std::fs::create_dir_all(folder.join("skills/notes")).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        "[package]\nname = { en = \"Kit\" }\n",
    )
    .expect("写得进");
    std::fs::write(
        folder.join("skills/notes/SKILL.md"),
        "---\nname: notes\ndescription: Take notes.\n---\nbody\n",
    )
    .expect("写得进");
    let install = json!({"install": folder}).to_string();
    let plays = vec![
        Play::calls(&[("shell", &pkg::run(&[("package", &install)]))]),
        Play::Says("装好了。"),
    ];
    let core = Arc::new(home.core_full(&Script::new(plays), pkg::catalog(), None, TOKEN));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &cwd).await;
    client.say("s1", &session, "装上它").await;
    until_waiting(&mut client, &session, true).await;
    let asked = until_logged(&home, &session, "tool.approval_requested").await;
    let call = asked["body"]["call_id"].clone();
    client
        .call(
            "a",
            "session.answer",
            json!({"session": session, "call": call, "decision": "once", "nested": 1}),
        )
        .await;
    home.until_turns(&session, 1).await;
    until_waiting(&mut client, &session, false).await;
}

/// 推送里也带（施工 SL-1）：订阅着会话列表的头，她开始问人时收到一条那一项带 `waiting: true` 的，答完了收到不带的。
#[tokio::test]
async fn the_sessions_stream_pushes_waiting_as_it_changes() {
    let home = Home::new();
    let args = json!({"questions": [{"question": "用哪个？", "options": [{"label": "甲"}, {"label": "乙"}]}]});
    let plays = vec![
        Play::calls(&[("ask_user", &args.to_string())]),
        Play::Says("好。"),
    ];
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).expect("读得出"))
        .expect("合写法");
    let core = home.core_with_tools(&Script::new(plays), tools, TOKEN);
    let mut watcher = Client::connect(Arc::clone(&core));
    watcher.hello().await;
    watcher
        .call("w", "subscribe", json!({"stream": "sessions"}))
        .await;
    let mut client = Client::connect(core);
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &cwd).await;
    client.say("s1", &session, "hi").await;
    loop {
        let entry = pushed(&mut watcher, &session).await;
        if entry["waiting"] == true {
            break;
        }
    }
    let call = until_logged(&home, &session, "question.asked").await["body"]["call_id"].clone();
    let params = json!({"session": session, "call": call, "answers": [{"picked": ["甲"]}]});
    client.call("a", "session.answer", params).await;
    loop {
        let entry = pushed(&mut watcher, &session).await;
        if entry.get("waiting").is_none() {
            break;
        }
    }
}

/// 等到会话 `session` 的日志里有一条 `kind`，交回它，最多一分钟：在等人的状态先变、那一条晚一点才落盘（列表照状态报，
/// 不等落盘），读早了会扑空（Windows 上偶发过）。
async fn until_logged(home: &Home, session: &str, kind: &str) -> Value {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        if let Some(event) = logged(home, session)
            .into_iter()
            .find(|event| event["kind"] == kind)
        {
            return event;
        }
        assert!(tokio::time::Instant::now() < deadline, "等不到 {kind}");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

/// 读推送，直到会话 `session` 的一条 `sessions.changed`，交回那一项。
async fn pushed(watcher: &mut Client, session: &str) -> Value {
    loop {
        let next = watcher.next().await.expect("十秒内推了");
        if next["method"] == "sessions.changed" && next["params"]["session"] == json!(session) {
            return next["params"]["entry"].clone();
        }
    }
}
