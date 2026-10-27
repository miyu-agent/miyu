//! 提示词的同名覆盖（施工 OV-1 中）：`home/<账号>/prompts/<路径>` 压着 `system/prompts/<路径>` 压着出厂的，开会话拼 system 时
//! 照它；占位不对、不是英文的照下一层，`check` 报。

use serde_json::json;

use miyu_kernel::event::Body;
use miyu_policy::Snapshot;
use miyu_session::testkit::Script;
use miyu_store::blob::Blobs;

use crate::support::*;

/// 会话 `session` 造的时候的 system。
fn system(home: &Home, session: &str) -> String {
    let policy = home
        .log(session)
        .iter()
        .find_map(|event| match &event.body {
            Body::SessionCreated(created) => Some(created.policy.clone()),
            _ => None,
        })
        .expect("有快照");
    let bytes = Blobs::new(home.root.blobs(&alice()))
        .get(&policy)
        .expect("快照在 blob 里");
    Snapshot::from_bytes(&bytes).expect("读得懂").system
}

#[tokio::test]
async fn prompt_overrides_go_into_new_sessions_and_bad_ones_are_reported() {
    let home = Home::new();
    home.write(
        "system/prompts/core/agent-version.txt",
        "Running on miyu-agent {version}.\n",
    );
    home.write(
        "system/prompts/core/local-paths-rule.txt",
        "System rule for local paths.\n",
    );
    home.write(
        "home/alice/prompts/core/local-paths-rule.txt",
        "Always write absolute paths for local files.\n",
    );
    home.write(
        "home/alice/prompts/core/agent-version.txt",
        "No placeholder here.\n",
    );
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let text = system(&home, &session);
    assert!(
        text.contains("Running on miyu-agent "),
        "自己的占位不对，照系统的：{text}"
    );
    assert!(!text.contains("No placeholder here."), "{text}");
    assert!(
        text.contains("Always write absolute paths for local files."),
        "自己的：{text}"
    );
    assert!(
        !text.contains("When a reply links or embeds"),
        "出厂的被盖掉了：{text}"
    );
    assert!(
        !text.contains("System rule for local paths."),
        "自己的压着系统的：{text}"
    );

    let checked = client.call("k1", "check", json!({})).await;
    let bad: Vec<&str> = checked["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{checked}"))
        .iter()
        .filter(|one| one["code"] == "prompt_override_bad")
        .filter_map(|one| one["file"].as_str())
        .collect();
    assert_eq!(bad.len(), 1, "{checked}");
    assert!(bad[0].ends_with("agent-version.txt"), "{bad:?}");
}

/// 会话 `session` 现在的快照（最后一次换上的）的 system。
fn latest_system(home: &Home, session: &str) -> String {
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
    let bytes = Blobs::new(home.root.blobs(&alice()))
        .get(&policy)
        .expect("快照在 blob 里");
    Snapshot::from_bytes(&bytes).expect("读得懂").system
}

/// 载入的会话（核心重启以后）换快照时也照属主的覆盖拼：预设的文件改了，下一轮重拼的 system 用上他写的那一份。
#[tokio::test]
async fn a_loaded_session_rebuilds_with_the_owner_s_overrides() {
    let home = Home::new();
    home.write(
        "home/alice/presets/p.toml",
        "[preset]\nname = { en = \"P\" }\n",
    );
    let session = {
        let mut client = Client::connect(home.core(&Script::new([])));
        client.hello().await;
        client
            .create_with("c1", json!({"cwd": "~", "preset": "p"}))
            .await
    };
    assert!(!latest_system(&home, &session).contains("Mine about local paths."));
    home.write(
        "home/alice/prompts/core/local-paths-rule.txt",
        "Mine about local paths.\n",
    );
    home.write(
        "home/alice/presets/p.toml",
        "[preset]\nname = { en = \"P2\" }\n",
    );
    let mut client =
        Client::connect(home.core(&Script::new([miyu_session::testkit::Play::Says("嗯。")])));
    client.hello().await;
    client.say("s1", &session, "在吗").await;
    home.until_turns(&session, 1).await;
    let text = latest_system(&home, &session);
    assert!(text.contains("Mine about local paths."), "{text}");
}
