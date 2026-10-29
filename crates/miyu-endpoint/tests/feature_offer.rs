//! 扩展交预设功能（施工 IM-6，接入通讯平台设计 06 第 4.6 节第 4 样）：`feature.offer` 交的功能进预设菜单、归交的
//! 包；列出来的工具归它，不归包自己的功能，开不开照预设；开着的会话下一个回合照新的算。工具不是自己登记的、编号撞了装了的
//! 功能的回 `bad_params`。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_policy::Snapshot;
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;

use crate::support::extensions::*;
use crate::support::*;

/// 等到 `path` 里记下的回应有 `n` 行（`cwd:` 那一行不算），交回它们。
async fn answers(path: &Path, n: usize) -> Vec<Value> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let got: Vec<Value> = read(path)
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        if got.len() >= n {
            return got;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {n} 行：{got:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 会话 `session` 现在的快照（最后一次换上的）里，`qq_*` 那几件工具，照名字排。
fn face(home: &Home, session: &str) -> Vec<String> {
    let policy = home
        .log(session)
        .iter()
        .filter_map(|event| match &event.body {
            Body::SessionCreated(created) => Some(created.policy.clone()),
            Body::PolicyChanged(changed) => changed.policy.clone(),
            _ => None,
        })
        .next_back()
        .expect("有快照");
    let bytes = Blobs::new(home.root.blobs())
        .get(&policy)
        .expect("快照在 blob 里");
    let mut names: Vec<String> = Snapshot::from_bytes(&bytes)
        .expect("读得懂")
        .tools
        .into_iter()
        .map(|tool| tool.name)
        .filter(|name| name.starts_with("qq_"))
        .collect();
    names.sort();
    names
}

/// 预设 `preset` 的菜单里编号是 `id` 的那一项。
async fn item(client: &mut Client, preset: &str, id: &str) -> Value {
    let got = client
        .call("g", "preset.get", json!({"preset": preset}))
        .await;
    got["result"]["features"]
        .as_array()
        .and_then(|all| all.iter().find(|one| one["id"] == id))
        .cloned()
        .unwrap_or_else(|| panic!("没有 {id}：{got}"))
}

#[tokio::test]
async fn offered_features_take_their_tools_into_the_preset_from_the_next_turn() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "each");
    let go = home.work.join("go");
    let tool = |name: &str| {
        json!({"name": name, "description": "A test tool.", "input_schema": {"type": "object"},
               "access": "read", "venues": ["local", "group", "private"]})
    };
    let provide = json!({"tools": [tool("qq_history"), tool("qq_send")]});
    let offer = |id: &str, tools: &[&str]| {
        format!(
            "ask:feature.offer:{}",
            json!({"features": [{"id": id, "name": {"en": "QQ history", "zh": "查询 QQ 群聊记录"},
                                 "tools": tools}]})
        )
    };
    install_providing(
        &home,
        "each",
        &program.name(),
        "always",
        &steps(&[
            &step,
            "hello",
            &format!("ask:provide:{provide}"),
            &format!("pause:{}", go.display()),
            &offer("qq-history", &["shell"]),
            &offer("each", &["qq_history"]),
            &offer("qq-history", &["qq_history"]),
            "wait",
        ]),
    );
    // 只开包自己的功能、没列的都关着：交以前 qq_history 归包的功能、开着；交了以后归交的功能、关着。
    home.write(
        "presets/p.toml",
        "[preset]\nname = { en = \"P\" }\nunlisted = \"off\"\n\n[features]\neach = true\n",
    );
    let core = core_with_script(
        &home,
        quick(),
        &Script::new([Play::Says("一。"), Play::Says("二。")]),
    );
    answers(&path, 2).await;
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client
        .create_with("c1", json!({"cwd": "~", "preset": "p"}))
        .await;
    client.say("s1", &session, "一").await;
    until_turns_in(&dir(&home, &session), 1).await;
    assert_eq!(face(&home, &session), ["qq_history", "qq_send"], "交以前");

    std::fs::write(&go, "").expect("写得进");
    let got = answers(&path, 5).await;
    assert_eq!(
        got[2]["error"]["data"]["field"], "features",
        "工具不是自己的：{got:?}"
    );
    assert_eq!(
        got[3]["error"]["data"]["field"], "features",
        "撞了包的功能：{got:?}"
    );
    assert_eq!(got[4]["result"], json!({}), "{got:?}");

    // 预设菜单：归交的包，名字照语言挑；全部功能开着，没列它的关着。
    let mine = item(&mut client, "p", "qq-history").await;
    assert_eq!(mine["package"], "each", "{mine}");
    assert_eq!(mine["name"], "查询 QQ 群聊记录", "照连接的语言挑：{mine}");
    assert_eq!(mine["on"], false, "{mine}");
    assert_eq!(mine["tools"][0]["name"], "qq_history", "{mine}");
    assert_eq!(item(&mut client, "full", "qq-history").await["on"], true);

    // 开着的会话下一个回合照新的算。
    client.say("s2", &session, "二").await;
    until_turns_in(&dir(&home, &session), 2).await;
    assert_eq!(face(&home, &session), ["qq_send"], "交了以后");
    core.stop_extensions().await;
}
