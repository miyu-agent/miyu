//! 注入环境信息（施工 ENV-1）：环境那几块和「关于你」在内置包 `env` 里。整包停用和预设里八项都关拼出来的 system 一字不差；只关
//! 「用户身份」的只少「关于你」那一块，别的一字不变。`preset.get` 的每个功能带它归哪个包。

use std::sync::Arc;

use serde_json::json;

use miyu_kernel::event::Body;
use miyu_policy::Snapshot;
use miyu_policy::shown::Shown;
use miyu_session::testkit::Script;
use miyu_store::blob::Blobs;

use crate::support::*;

/// 写进 alice 个人设置的称呼。
const PROFILE: &str = "[profile]\nname = \"小林\"\n";

/// 「关于你」那一块。
const BLOCK: &str = "<user>\nname: 小林\n</user>";

/// 预设：没列出来的照开，`off` 里的几个功能关掉。
fn preset(off: &[&str]) -> String {
    let lines: Vec<String> = off.iter().map(|id| format!("{id} = false")).collect();
    format!(
        "[preset]\nname = {{ en = \"Test\" }}\n\n[features]\n{}\n",
        lines.join("\n")
    )
}

/// 会话 `session` 的策略快照。
fn snapshot(home: &Home, session: &str) -> Snapshot {
    let log = home.log(session);
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第 1 条应该是造会话");
    };
    let bytes = Blobs::new(home.root.blobs(&alice()))
        .get(&created.policy)
        .expect("快照在 blob 里");
    Snapshot::from_bytes(&bytes).expect("读得懂")
}

/// 用预设 `preset` 开一个会话，交回它的快照。
async fn opened(client: &mut Client, home: &Home, id: &str, preset: &str) -> Snapshot {
    let cwd = home.work.to_string_lossy().into_owned();
    let session = client
        .create_with(id, json!({"cwd": cwd, "preset": preset}))
        .await;
    snapshot(home, &session)
}

#[tokio::test]
async fn switching_the_package_off_is_byte_for_byte_like_turning_all_eight_off() {
    let home = Home::new();
    home.write("home/alice/settings.toml", PROFILE);
    home.write(
        "home/alice/presets/noenv.toml",
        &preset(&miyu_policy::shown::ALL),
    );
    home.write(
        "home/alice/presets/noprofile.toml",
        &preset(&[miyu_policy::shown::PROFILE]),
    );
    let core = home.core_configured(&Script::new([]), None, &[]);
    let mut client = Client::connect(Arc::clone(&core));
    client
        .call("h", "hello", json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0.0.0"}, "caps": {"input": true}, "token": TOKEN}))
        .await;
    let full = opened(&mut client, &home, "c1", "full").await;
    assert!(
        full.system.contains(&format!("{BLOCK}\n\n")),
        "开着的有关于你：{}",
        full.system
    );
    let noprofile = opened(&mut client, &home, "c2", "noprofile").await;
    assert_eq!(noprofile.profile, None, "只关用户身份");
    assert_eq!(
        noprofile.system,
        full.system.replace(&format!("{BLOCK}\n\n"), ""),
        "只少关于你那一块，别的一字不变"
    );
    let all_off = opened(&mut client, &home, "c3", "noenv").await;
    assert!(!all_off.system.contains("<user>"), "{}", all_off.system);
    assert_ne!(all_off.system, noprofile.system, "环境那几行也不拼");

    let disabled = client
        .call("d", "package.disable", json!({"package": "env"}))
        .await;
    assert!(disabled["result"].is_object(), "{disabled}");
    let gone = opened(&mut client, &home, "c4", "full").await;
    assert_eq!(gone.system, all_off.system, "整包停用和八项都关一字不差");
    assert_eq!(
        Shown::of(gone.preset.as_ref()),
        Shown::of(all_off.preset.as_ref()),
        "回合开始注入的事实也一样关"
    );
    let gone_noenv = opened(&mut client, &home, "c5", "noenv").await;
    assert_eq!(gone_noenv.system, all_off.system);
}

/// `preset.get` 里功能 `id` 的那一格 `package`；没有这个功能的报错，没有这一格的是 `None`。
fn package_of<'a>(got: &'a serde_json::Value, id: &str) -> Option<&'a str> {
    got["result"]["features"]
        .as_array()
        .expect("是一个个功能")
        .iter()
        .find(|one| one["id"] == id)
        .unwrap_or_else(|| panic!("没有功能 {id}：{got}"))
        .get("package")
        .map(|package| package.as_str().expect("是字"))
}

#[tokio::test]
async fn preset_get_says_which_package_each_feature_is_in() {
    let home = Home::new();
    home.write(
        "home/alice/presets/noenv.toml",
        &preset(&miyu_policy::shown::ALL),
    );
    let core = home.core_configured(&Script::new([]), None, &[]);
    let mut client = Client::connect(Arc::clone(&core));
    client
        .call("h", "hello", json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0.0.0"}, "caps": {"input": true}, "token": TOKEN}))
        .await;
    let got = client
        .call("g1", "preset.get", json!({"preset": "full"}))
        .await;
    for id in miyu_policy::shown::ALL {
        assert_eq!(package_of(&got, id), Some("env"), "{id}");
    }
    assert_eq!(
        package_of(&got, "read"),
        Some("read"),
        "施工 BS-1 上中起一件一个包"
    );

    client
        .call("d", "package.disable", json!({"package": "env"}))
        .await;
    let got = client
        .call("g2", "preset.get", json!({"preset": "noenv"}))
        .await;
    assert_eq!(
        got["result"]["features"]
            .as_array()
            .expect("是一个个功能")
            .iter()
            .find(|one| one["id"] == miyu_policy::shown::TIME)
            .map(|one| one["installed"].clone()),
        Some(json!(false)),
        "{got}"
    );
    assert_eq!(
        package_of(&got, miyu_policy::shown::TIME),
        None,
        "没装的没有这一格"
    );
}
