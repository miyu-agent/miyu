//! 留言照项目筛（施工 H-3，设计 `38-项目.md` 第五节）：真核心里她调 `sessions`，项目没开 `open` 的只看得到同一个项目里的会话，
//! 外面的看不到；项目开了的连外面的也看得到。认会话、读、发话都照这一批认，看不到的就是没有。

use std::sync::Arc;

use serde_json::json;

use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};

use crate::support::deleting::base_tools;
use crate::support::*;

/// 会话 `session` 的日志里第一条工具结果的字。
fn first_result(home: &Home, session: &str) -> String {
    home.log(session)
        .into_iter()
        .find_map(|event| match event.body {
            Body::ToolResult(result) => Some(format!("{:?}", result.blocks)),
            _ => None,
        })
        .expect("有一条工具结果")
}

/// 编号的后 8 位：列出来的时候至少写这么多。
fn tail(session: &str) -> &str {
    &session[session.len() - 8..]
}

#[tokio::test]
async fn a_closed_project_sees_only_its_own_and_an_open_one_sees_outside() {
    let home = Home::new();
    let plays = [
        Play::calls(&[("sessions", "{}")]),
        Play::Says("好。"),
        Play::calls(&[("sessions", "{}")]),
        Play::Says("好。"),
        Play::calls(&[("sessions", "{}")]),
        Play::Says("好。"),
    ];
    let core = home.core_with_tools(&Script::new(plays), base_tools(), TOKEN);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let made = client
        .call("p", "project.create", json!({"name": "Closed"}))
        .await;
    let project = made["result"]["project"]
        .as_str()
        .expect("有编号")
        .to_string();
    let inside = client
        .create_with("c1", json!({"cwd": "~", "project": project}))
        .await;
    let neighbour = client
        .create_with("c2", json!({"cwd": "~", "project": project}))
        .await;
    let outside = client.create("c3", "~").await;

    client.say("s1", &inside, "列一下会话").await;
    home.until_turns(&inside, 1).await;
    let seen = first_result(&home, &inside);
    assert!(
        seen.contains(tail(&neighbour)),
        "同一个项目的看得到：{seen}"
    );
    assert!(!seen.contains(tail(&outside)), "项目外的看不到：{seen}");

    client
        .call(
            "u",
            "project.update",
            json!({"project": project, "open": true}),
        )
        .await;
    client.say("s2", &outside, "列一下会话").await;
    home.until_turns(&outside, 1).await;
    let seen = first_result(&home, &outside);
    assert!(
        seen.contains(tail(&inside)) && seen.contains(tail(&neighbour)),
        "开了的项目和外面互相看得到：{seen}"
    );

    // 文件里没有了这个项目（手改了）：当不在项目里，和外面的人人互相看得到。
    let file = home.root.path().join("projects.json");
    std::fs::write(&file, "{\"version\":1,\"projects\":[]}\n").expect("写得进");
    client.say("s3", &inside, "再列一下").await;
    home.until_turns(&inside, 2).await;
    let seen = results(&home, &inside).pop().expect("又有一条");
    assert!(
        seen.contains(tail(&outside)),
        "项目没了的当不在项目里：{seen}"
    );
}

/// 会话 `session` 的日志里每一条工具结果的字，照先后。
fn results(home: &Home, session: &str) -> Vec<String> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(format!("{:?}", result.blocks)),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn the_project_prompt_follows_the_session_from_the_next_turn() {
    let home = Home::new();
    let script = Script::new([Play::Says("一。"), Play::Says("二。"), Play::Says("三。")]);
    let core = home.core(&script);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let made = client
        .call(
            "p",
            "project.create",
            json!({"name": "Miyu", "prompt": "Reply in Chinese."}),
        )
        .await;
    let project = made["result"]["project"]
        .as_str()
        .expect("有编号")
        .to_string();
    let session = client
        .create_with("c", json!({"cwd": "~", "project": project}))
        .await;
    let block = "<project_instructions>\nReply in Chinese.\n</project_instructions>";

    client.say("s1", &session, "你好").await;
    home.until_turns(&session, 1).await;
    client
        .call(
            "u",
            "project.update",
            json!({"project": project, "prompt": "Reply in Japanese."}),
        )
        .await;
    client.say("s2", &session, "再说").await;
    home.until_turns(&session, 2).await;
    client
        .call(
            "o",
            "session.set_meta",
            json!({"session": session, "project": null}),
        )
        .await;
    client.say("s3", &session, "最后").await;
    home.until_turns(&session, 3).await;

    let systems: Vec<String> = script
        .requests()
        .into_iter()
        .map(|(_, request)| request.system)
        .collect();
    assert_eq!(systems.len(), 3);
    assert!(systems[0].contains(block), "第一轮就带上：{}", systems[0]);
    assert!(
        systems[1].contains("Reply in Japanese.") && !systems[1].contains("Reply in Chinese."),
        "项目改了，下一轮换上：{}",
        systems[1]
    );
    assert!(
        !systems[2].contains("<project_instructions>"),
        "移出项目，下一轮没有：{}",
        systems[2]
    );
}
