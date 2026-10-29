//! `venue.sessions`（施工 O-32 前，`venues.md`「列场所会话」）：核心拉起的平台桥（清单写着 `[connection] platform = "qq"`）列出
//! 它接的平台的场所会话，每个场所一个（施工 SU-4 中起场所会话都归主人）：删了的不列；同一个场所有几个的（从回收处拿回来的旧的、
//! 私聊的对方后来写进了对应表另开的）只列最新的；别的平台的、桥自己造的本机会话不列。多写格的 `bad_params`；本机的头（不是桥）
//! 回 `needs_approval`（没批 `act_for_external`）。同一个数据根上换一份核心，就像重启过。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::venues::BINDINGS;
use crate::support::*;

/// 系统配置写成 `config`、读好的清单起来的核心，拉起开着的扩展。
fn served_core(home: &Home, config: &str) -> Arc<Core> {
    home.write("config.toml", config);
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
        miyu_endpoint::settings::EXTERNAL_BINDINGS,
    ]
    .concat();
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let found = miyu_endpoint::packages::load(&resources, &home.root);
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        None,
        items,
        miyu_endpoint::config::Environment::of(&[]),
    );
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick())
            .with_config(config)
            .with_packages(found),
    );
    core.start_extensions();
    core
}

/// 等到记下的回应有 `n` 行，交回它们。
async fn replies(path: &Path, n: usize) -> Vec<Value> {
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

/// 回应里的会话编号。
fn made(reply: &Value) -> String {
    reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("有编号：{reply}"))
        .to_string()
}

/// 平台桥 `bot` 的清单：程序 `program` 照 `args` 一步步做，开着就拉起，接 `qq`。
fn install_bridge(home: &Home, program: &str, args: &[String]) {
    let args: Vec<String> = args.iter().map(|arg| format!("{arg:?}")).collect();
    home.write(
        "packages/bot/package.toml",
        &format!(
            "[package]\nprotocol = [1, 1]\nname = {{ en = \"Bridge\" }}\n\n[command]\nname = \"bot\"\nprogram = \"{program}\"\nabout = {{ en = \"B\" }}\n\n[process]\nargs = [{}]\nstart = \"always\"\ncapabilities = {BRIDGE:?}\n\n[connection]\nplatform = \"qq\"\n",
            args.join(", ")
        ),
    );
    approve(home, "bot", &BRIDGE);
}

/// 一份核心起来、系统配置写成 `config`，扩展照 `steps`（前面补上记下和握手）做完，交回记下的回应（`n` 行），核心停下。
async fn phase(
    home: &Home,
    program: &Program,
    name: &str,
    config: &str,
    steps: &[&str],
    n: usize,
) -> Vec<Value> {
    let (path, step) = record(home, name);
    // 每份核心的请求编号各带前缀：核心照命令编号认重发的，编号撞了会交回上一份造的。
    let ids = format!("ids:{name}-");
    let all: Vec<String> = [step.as_str(), &ids, "hello"]
        .iter()
        .chain(steps)
        .chain(&["wait"])
        .map(ToString::to_string)
        .collect();
    install_bridge(home, &program.name(), &all);
    let core = served_core(home, config);
    let got = replies(&path, n).await;
    assert!(got[0].get("result").is_some(), "握手成了：{got:?}");
    if name == "first" {
        // 本机的头不是桥。
        let mut client = Client::connect(Arc::clone(&core));
        client.hello().await;
        let refused = client.call("l1", "venue.sessions", json!({})).await;
        assert_eq!(reason(&refused), Some("needs_approval"), "{refused}");
    }
    core.stop_extensions().await;
    core.stop_sessions().await;
    got
}

/// 造场所 `venue` 的会话：群的不写对方；私聊的对方是 `<平台>:<号>`。
fn open(venue: &str) -> String {
    let parts: Vec<&str> = venue.split(':').collect();
    match parts.as_slice() {
        [platform, "private", number] => format!(
            r#"ask:venue.session:{{"venue":"{venue}","kind":"private","peer":"{platform}:{number}"}}"#
        ),
        _ => format!(r#"ask:venue.session:{{"venue":"{venue}","kind":"group"}}"#),
    }
}

/// 列出来的 `{session, venue}`，照给的先后。
fn listed(pairs: &[(&str, &str)]) -> Value {
    let sessions: Vec<Value> = pairs
        .iter()
        .map(|(session, venue)| json!({"session": session, "venue": venue}))
        .collect();
    json!({"sessions": sessions})
}

#[tokio::test]
async fn a_bridge_lists_the_venue_sessions_of_its_platform() {
    let home = Home::new();
    let program = Program::new();
    let config = format!("{BINDINGS}\"tg:30001\" = \"alice\"\n");
    let got = phase(
        &home,
        &program,
        "first",
        &config,
        &[
            &open("qq:group:1"),
            // 删群会话是桥删（施工 Y-2 上起人的头碰不到场所会话，项目主人 10-23 定删放在平台的后台）。
            r#"ask:session.delete:{"session":"{session}"}"#,
            &open("qq:group:1"),
            &open("qq:group:2"),
            &open("qq:private:20001"),
            &open("qq:private:10001"),
            &open("tg:private:30001"),
            r#"ask:session.create:{"cwd":"~"}"#,
            "call:venue.sessions",
            r#"ask:venue.sessions:{"more":1}"#,
        ],
        11,
    )
    .await;
    let deleted = made(&got[1]);
    assert_eq!(got[2]["result"], json!({}), "桥删得掉它的平台的群：{got:?}");
    assert_eq!(got[3]["result"]["created"], true, "删了的找不回来，另造");
    let (first, second, stranger, admins) =
        (made(&got[3]), made(&got[4]), made(&got[5]), made(&got[6]));
    made(&got[7]);
    made(&got[8]);
    assert_eq!(
        got[9]["result"],
        listed(&[
            (&admins, "qq:private:10001"),
            (&stranger, "qq:private:20001"),
            (&second, "qq:group:2"),
            (&first, "qq:group:1"),
        ]),
        "从新到旧；删了的、别的平台的、桥自己造的本机会话不列：{got:?}"
    );
    assert_eq!(reason(&got[10]), Some("bad_params"), "{}", got[10]);

    // 删了的那一个从回收处拿回来：同一个群有两个，只列新的。陌生人后来写进了对应表、私聊又来了一句：他成了主人本人，另开一个
    // 不关沙盒的，列那一个，关着的旧的不列。
    let trashed = home.root.trashed_sessions().join(&deleted);
    let restored = home.root.path().join("sessions").join(&deleted);
    std::fs::remove_file(trashed.join(miyu_store::trash::DELETED_AT)).expect("删得掉");
    std::fs::rename(&trashed, &restored).expect("挪得回来");
    let rebound = format!("{BINDINGS}\"qq:20001\" = \"alice\"\n");
    let got = phase(
        &home,
        &program,
        "second",
        &rebound,
        &[&open("qq:private:20001"), "call:venue.sessions"],
        3,
    )
    .await;
    let moved = made(&got[1]);
    assert_ne!(moved, stranger, "另造了一个");
    assert_eq!(
        got[2]["result"],
        listed(&[
            (&moved, "qq:private:20001"),
            (&admins, "qq:private:10001"),
            (&second, "qq:group:2"),
            (&first, "qq:group:1"),
        ]),
        "{got:?}"
    );

    // 对应表里没有这个平台的身份了：场所会话照旧是这个平台的，照旧列（谁关沙盒等对方再来一句由 `venue.session` 照对应表算）。
    let got = phase(
        &home,
        &program,
        "third",
        "[external.bindings]\n\"tg:30001\" = \"alice\"\n",
        &["call:venue.sessions"],
        2,
    )
    .await;
    assert_eq!(
        got[1]["result"],
        listed(&[
            (&moved, "qq:private:20001"),
            (&admins, "qq:private:10001"),
            (&second, "qq:group:2"),
            (&first, "qq:group:1"),
        ]),
        "{got:?}"
    );
}
