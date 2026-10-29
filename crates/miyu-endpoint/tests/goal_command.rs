//! 斜杠命令 `/goal`（施工 X-8 下，`docs/blueprint/goal.md`「在新的架构里怎么放」第 6、8 条）：真核心、假模型。设了就开续轮，
//! 看、停、改、继续、清掉各一句回执，记 `goal.*`（`by` 是打命令的人）和 `command.ran`；卡住的改了上限能接着跑；不合的照内核的
//! 原因拒；没有长期目标的会话（工具面上没有、预设关了、通讯平台的）拒 `goal_unavailable`，`data.why` 说具体。

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, Event};
use miyu_kernel::origin::By;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::venues::{bound_core, configured_core};
use crate::support::*;

/// 有长期目标的工具的目录：记着归哪个包，预设才关得掉它。
pub(crate) fn goal_tools() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools = miyu_goal::tools(&resources).expect("读得出");
    Catalog::in_packages([(miyu_goal::PACKAGE, tools)]).expect("合写法")
}

async fn run(client: &mut Client, id: &str, session: &str, text: &str) -> Value {
    client
        .call(id, "command.run", json!({"session": session, "text": text}))
        .await
}

/// 回执那一句。
fn said(reply: &Value) -> &str {
    reply["result"]["said"].as_str().unwrap_or_default()
}

/// 等到日志里有一条合 `wanted` 的：最多六十秒。
async fn until(home: &Home, session: &str, wanted: impl Fn(&[Event]) -> bool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while !wanted(&home.log(session)) {
        assert!(
            tokio::time::Instant::now() < deadline,
            "六十秒内没等到：{:?}",
            home.log(session)
                .iter()
                .map(|event| event.body.kind())
                .collect::<Vec<_>>()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// 日志里结束了 `turns` 轮以后，这一批也送完了（续轮看完一眼）。
async fn until_ended(home: &Home, session: &str, turns: usize) {
    until(home, session, |log| {
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            >= turns
    })
    .await;
}

/// 日志里长期目标的事件，照先后。
fn goal_kinds(log: &[Event]) -> Vec<&str> {
    log.iter()
        .map(|event| event.body.kind())
        .filter(|kind| kind.starts_with("goal."))
        .collect()
}

#[tokio::test]
async fn set_show_pause_edit_resume_and_clear() {
    let home = Home::new();
    let script = Script::new([Play::Says("在等你确认。"), Play::Says("还在等。")])
        .titles([Play::Says("测试")]);
    let mut client = Client::connect(configured_core(&home, &script, goal_tools()));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = run(&mut client, "k1", &session, "/goal").await;
    assert_eq!(said(&reply), "无长期目标。", "{reply}");

    let reply = run(&mut client, "k2", &session, "/goal --rounds 3 把测试修绿").await;
    assert_eq!(reply["result"]["command"], "goal", "{reply}");
    assert_eq!(said(&reply), "已设定长期目标：把测试修绿");
    // 设了就开续轮：这一轮没调工具，停下等人。
    until_ended(&home, &session, 1).await;
    let reply = run(&mut client, "k3", &session, "/goal").await;
    assert_eq!(said(&reply), "把测试修绿 · 等待回复 · 第 1/3 轮");

    let reply = run(&mut client, "k4", &session, "/goal pause").await;
    assert_eq!(said(&reply), "已暂停长期目标。");
    let reply = run(&mut client, "k5", &session, "/goal pause").await;
    assert_eq!(reason(&reply), Some("goal_paused"), "{reply}");
    let reply = run(&mut client, "k6", &session, "/goal edit 再写\n文档").await;
    assert_eq!(said(&reply), "已修改长期目标：再写 文档", "换行写成空格");
    let reply = run(&mut client, "k7", &session, "/goal").await;
    assert_eq!(
        said(&reply),
        "再写 文档 · 已暂停 · 第 1/3 轮",
        "没写上限的照原来的"
    );
    let reply = run(&mut client, "k9", &session, "/goal resume").await;
    assert_eq!(said(&reply), "已继续长期目标。");
    until_ended(&home, &session, 2).await;
    let reply = run(&mut client, "k10", &session, "/goal").await;
    assert_eq!(said(&reply), "再写 文档 · 等待回复 · 第 2/3 轮");

    let reply = run(&mut client, "k11", &session, "/goal clear").await;
    assert_eq!(said(&reply), "已清除长期目标。");
    let reply = run(&mut client, "k12", &session, "/goal").await;
    assert_eq!(said(&reply), "无长期目标。");

    let log = home.log(&session);
    assert_eq!(
        goal_kinds(&log),
        [
            "goal.set",
            "goal.paused",
            "goal.changed",
            "goal.resumed",
            "goal.ended"
        ]
    );
    assert!(
        log.iter()
            .filter(|event| event.body.kind().starts_with("goal."))
            .all(|event| matches!(&event.by, By::Person(_))),
        "by 是打命令的人"
    );
    let ran = log
        .iter()
        .filter(|event| matches!(event.body, Body::CommandRan(_)))
        .count();
    assert_eq!(ran, 10, "执行了的都记，被拒的一次不记");
}

/// 已经有一个没完成的再发 `/goal <目标>` 就是改它（施工 X-8 再补，2026-10-17 项目主人定）：原文换掉、第几轮接着数，没写
/// `--rounds` 的上限照原来的；停着的改完接着跑；等人的只改不另开；清掉以后才是新设一个。
#[tokio::test]
async fn a_second_goal_edits_the_current_one() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("在等你确认。"),
        Play::Says("还在等。"),
        Play::Says("好。"),
    ])
    .titles([Play::Says("测试")]);
    let mut client = Client::connect(configured_core(&home, &script, goal_tools()));
    client.hello().await;
    let session = client.create("c1", "~").await;
    run(&mut client, "k1", &session, "/goal --rounds 3 把测试修绿").await;
    until_ended(&home, &session, 1).await;
    run(&mut client, "k2", &session, "/goal pause").await;
    let reply = run(&mut client, "k3", &session, "/goal 再写文档").await;
    assert_eq!(said(&reply), "已修改长期目标：再写文档", "{reply}");
    until_ended(&home, &session, 2).await;
    let reply = run(&mut client, "k4", &session, "/goal").await;
    assert_eq!(
        said(&reply),
        "再写文档 · 等待回复 · 第 2/3 轮",
        "停着的改完接着跑，轮数接着数，上限照原来的"
    );
    let reply = run(&mut client, "k5", &session, "/goal --rounds 5 再补测试").await;
    assert_eq!(said(&reply), "已修改长期目标：再补测试");
    let reply = run(&mut client, "k6", &session, "/goal").await;
    assert_eq!(
        said(&reply),
        "再补测试 · 等待回复 · 第 2/5 轮",
        "等人的只改"
    );
    run(&mut client, "k7", &session, "/goal clear").await;
    let reply = run(&mut client, "k8", &session, "/goal 新的").await;
    assert_eq!(said(&reply), "已设定长期目标：新的", "清掉以后是新设");
    let log = home.log(&session);
    assert_eq!(
        goal_kinds(&log),
        [
            "goal.set",
            "goal.paused",
            "goal.changed",
            "goal.resumed",
            "goal.changed",
            "goal.ended",
            "goal.set"
        ]
    );
}

#[tokio::test]
async fn a_blocked_goal_resumes_after_its_limit_is_lifted() {
    let home = Home::new();
    let script = Script::new([
        Play::calls(&[("goal", r#"{"action":"get"}"#)]),
        Play::Says("看了。"),
        Play::Says("接着做。"),
    ])
    .titles([Play::Says("测试")]);
    let mut client = Client::connect(configured_core(&home, &script, goal_tools()));
    client.hello().await;
    let session = client.create("c1", "~").await;
    run(&mut client, "k1", &session, "/goal --rounds 1 把测试修绿").await;
    // 第 1 轮调了工具：到了上限，记卡住。
    until(&home, &session, |log| {
        log.iter()
            .any(|event| matches!(event.body, Body::GoalEnded(_)))
    })
    .await;
    let reply = run(&mut client, "k2", &session, "/goal").await;
    assert_eq!(
        said(&reply),
        "把测试修绿 · 受阻：Reached the round limit (1). · 第 1/1 轮",
        "{reply}"
    );
    let reply = run(&mut client, "k4", &session, "/goal pause").await;
    assert_eq!(reason(&reply), Some("goal_paused"), "{reply}");
    run(
        &mut client,
        "k5",
        &session,
        "/goal edit --rounds 0 把测试修绿",
    )
    .await;
    let reply = run(&mut client, "k6", &session, "/goal resume").await;
    assert_eq!(said(&reply), "已继续长期目标。", "{reply}");
    until_ended(&home, &session, 2).await;
    let reply = run(&mut client, "k7", &session, "/goal").await;
    assert_eq!(said(&reply), "把测试修绿 · 等待回复 · 第 2 轮");
}

#[tokio::test]
async fn without_goals_it_is_refused_and_says_why() {
    let home = Home::new();
    let script = Script::new([Play::Says("在。")]);
    // 工具面上没有长期目标的会话：照拒绝的那一句。
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = run(&mut client, "k1", &session, "/goal 把测试修绿").await;
    assert_eq!(reason(&reply), Some("goal_unavailable"), "{reply}");
    assert_eq!(reply["error"]["data"]["why"], "此会话不支持长期目标。");

    // 预设关了长期目标的：带上预设的名字。
    let home = Home::new();
    home.write(
        "presets/nogoal.toml",
        "[preset]\nname = \"不带目标\"\n\n[features]\ngoal = false\n",
    );
    let mut client = Client::connect(configured_core(&home, &script, goal_tools()));
    client.hello().await;
    let session = client
        .create_with("c1", json!({"cwd": "~", "preset": "nogoal"}))
        .await;
    let reply = run(&mut client, "k1", &session, "/goal").await;
    assert_eq!(reason(&reply), Some("goal_unavailable"), "{reply}");
    assert_eq!(
        reply["error"]["data"]["why"],
        "预设「不带目标」未启用长期目标。"
    );
    let listed = client
        .call("c2", "command.catalog", json!({"session": session}))
        .await;
    assert!(
        !listed.to_string().contains("\"goal\""),
        "命令菜单不列：{listed}"
    );

    // 通讯平台的会话：不支持。
    let home = Home::new();
    let mut client = Client::connect(bound_core(&home, &script, goal_tools()));
    client.hello().await;
    let made = client
        .call(
            "v1",
            "venue.session",
            json!({"venue": "qq:private:10001", "kind": "private", "peer": "qq:10001"}),
        )
        .await;
    let session = made["result"]["session"].as_str().expect("有编号");
    let reply = client
        .call(
            "k1",
            "command.run",
            json!({"session": session, "text": "/goal 把测试修绿", "as": {"external": "qq:10001"}}),
        )
        .await;
    assert_eq!(reason(&reply), Some("goal_unavailable"), "{reply}");
    assert!(goal_kinds(&home.log(session)).is_empty());
}

#[tokio::test]
async fn the_menu_lists_it_where_it_works() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = Client::connect(configured_core(&home, &script, goal_tools()));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client
        .call("c2", "command.catalog", json!({"session": session}))
        .await;
    let goal = reply["result"]["commands"]
        .as_array()
        .expect("一个个命令")
        .iter()
        .find(|command| command["name"] == "goal")
        .cloned();
    assert_eq!(
        goal,
        Some(
            json!({"name": "goal", "aliases": [], "summary": "长期目标", "argument": "[<目标> | pause | resume | clear]", "usage": [
                {"form": "/goal", "summary": "看进度"},
                {"form": "/goal <目标>", "summary": "设定长期目标；已经有一个就改它"},
                {"form": "/goal pause", "summary": "暂停"},
                {"form": "/goal resume", "summary": "继续"},
                {"form": "/goal clear", "summary": "清除"},
            ]})
        ),
        "{reply}"
    );
}
