//! 会话列表（`protocol.md` 的 `session.list`、「会话列表的推送」）：桥开的会话也归主人，桥和主人列出来的是同一份（施工 SU-4 中）。

use serde_json::json;

use miyu_session::testkit::Script;

use crate::blob_session::{replies, served_core};
use crate::support::extensions::*;
use crate::support::*;

#[tokio::test]
async fn the_bridge_and_the_owner_list_the_same_sessions() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    let ready = home.work.join("ready");
    install_serving(
        &home,
        "bot",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            &format!("pause:{}", ready.display()),
            r#"ask:session.create:{"cwd":"~"}"#,
            r#"ask:session.list:{}"#,
            r#"ask:subscribe:{"stream":"sessions"}"#,
            "wait",
        ]),
        "always",
        true,
    );
    let core = served_core(&home, &Script::new([]));
    let mut client = Client::connect(core);
    client.hello().await;
    let mine = client.create("c1", "~").await;
    std::fs::write(&ready, "").expect("写得进");
    let read = replies(&path, 4).await;
    let theirs = read[1]["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{read:?}"))
        .to_string();
    let listed = |reply: &serde_json::Value| -> Vec<String> {
        reply["result"]["sessions"]
            .as_array()
            .unwrap_or_else(|| panic!("{reply}"))
            .iter()
            .filter_map(|item| item["session"].as_str().map(str::to_string))
            .collect()
    };
    let both = |mut got: Vec<String>| {
        got.sort();
        got
    };
    let mut expected = vec![mine, theirs];
    expected.sort();
    assert_eq!(both(listed(&read[2])), expected, "桥列的是主人的全部");
    assert_eq!(both(listed(&read[3])), expected, "桥订阅的列表也是");
    let reply = client.call("l1", "session.list", json!({})).await;
    assert_eq!(both(listed(&reply)), expected, "主人看得到桥开的");
}

/// 订阅着会话列表的桥自己开了会话，收得到那一项的推送（施工 Y-12 上）。
#[tokio::test]
async fn a_new_session_is_pushed_to_its_owner() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    install_serving(
        &home,
        "bot",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            r#"ask:subscribe:{"stream":"sessions"}"#,
            r#"ask:session.create:{"cwd":"~"}"#,
            "listen",
        ]),
        "always",
        true,
    );
    let _core = served_core(&home, &Script::new([]));
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let lines: Vec<serde_json::Value> = read(&path)
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        let created = lines
            .iter()
            .find_map(|line| line["result"]["session"].as_str().map(str::to_string));
        let pushed = lines.iter().any(|line| {
            line["method"] == "sessions.changed"
                && created
                    .as_deref()
                    .is_some_and(|created| line["params"]["entry"]["session"] == created)
        });
        if pushed {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到那一项的推送：{lines:?}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}
