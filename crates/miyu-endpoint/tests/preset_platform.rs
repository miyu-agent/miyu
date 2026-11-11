//! 接平台的包在预设菜单里（施工 PS-2，`docs/blueprint/presets.md`「功能分组」）：清单写了 `[connection]` 的包，`[features]` 里一件
//! `venues` 带 `local` 的工具都没有的功能不列，列出来的只列带 `local` 的工具；`sections`、`features` 两处一样。没写 `[connection]`
//! 的包照旧全列。预设里存着的、对藏掉的功能和工具的开关不报错、不碰别的项。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::tool::Access;
use miyu_session::testkit::Script;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool, Venues};

use crate::preset_sections::{get, rows, section};
use crate::support::extensions::Program;
use crate::support::venues::configured_core;
use crate::support::*;

/// 接平台的包的三项功能：全是私聊、群的，混着本机的，全是本机的。
const BRIDGE_FEATURES: &str = "\n[connection]\nplatform = \"qq\"\n\n[features.remote]\nname = { zh = \"群管理\" }\ntools = [\"qq_kick\", \"qq_mute\"]\n\n[features.mixed]\nname = { zh = \"聊天动作\" }\ntools = [\"qq_send\", \"qq_history\", \"qq_poke\"]\n\n[features.home]\nname = { zh = \"本机\" }\ntools = [\"note_a\", \"note_b\"]\n";

/// 不接平台的包：一项功能，两件工具都只给私聊。
const PLAIN_FEATURES: &str =
    "\n[features.chat]\nname = { zh = \"只给私聊\" }\ntools = [\"plain_say\", \"plain_ask\"]\n";

/// 一个扩展包的清单：程序 `program`，`rest` 接在 `[process]` 后面。
fn extension(home: &Home, id: &str, program: &Program, rest: &str) {
    home.write(
        &format!("opt/{id}/package.toml"),
        &format!(
            "[package]\nprotocol = [1, 1]\nname = {{ zh = \"{id}\" }}\n\n[command]\nname = \"{id}\"\nprogram = \"{}\"\nabout = {{ en = \"X\" }}\n\n[process]\n{rest}",
            program.name()
        ),
    );
}

/// 一件只给 `venues` 的假工具。
fn fake(name: &str, venues: Venues) -> Arc<dyn Tool> {
    Fake::in_venues(name, Access::Read, venues, Act::Echo)
}

/// 两个包登记好的工具。
fn catalog() -> Catalog {
    let (local, private, group) = (
        Venues {
            local: true,
            ..Venues::default()
        },
        Venues {
            private: true,
            ..Venues::default()
        },
        Venues {
            group: true,
            ..Venues::default()
        },
    );
    let both = Venues {
        private: true,
        group: true,
        ..Venues::default()
    };
    let local_group = Venues {
        local: true,
        group: true,
        ..Venues::default()
    };
    Catalog::in_packages([
        (
            "bridge",
            vec![
                fake("qq_kick", group),
                fake("qq_mute", both),
                fake("qq_send", local_group),
                fake("qq_history", private),
                fake("qq_poke", local),
                fake("note_a", local),
                fake("note_b", local_group),
            ],
        ),
        (
            "plain",
            vec![fake("plain_say", private), fake("plain_ask", private)],
        ),
    ])
    .expect("合写法")
}

/// 装好两个包、握好手的连接。
async fn connected(home: &Home, program: &Program) -> Client {
    extension(home, "bridge", program, BRIDGE_FEATURES);
    extension(home, "plain", program, PLAIN_FEATURES);
    let mut client = Client::connect(configured_core(home, &Script::new([]), catalog()));
    let hello = json!({
        "protocol": [1, 1],
        "head": {"kind": "test", "version": "0.0.0"},
        "locale": "zh-CN",
        "caps": {"input": true},
        "token": TOKEN,
    });
    client.call("hello-1", "hello", hello).await;
    client
}

/// `sections` 里功能 `id` 下面列的工具编号；没有 `tools` 的是空的。
fn tool_ids(result: &Value, id: &str) -> Vec<String> {
    let items = result["sections"]
        .as_array()
        .expect("有 sections")
        .iter()
        .flat_map(|one| one["items"].as_array().expect("一项项"))
        .find(|one| one["id"] == id)
        .unwrap_or_else(|| panic!("没有 {id}：{result}"));
    items["tools"]
        .as_array()
        .map(|tools| {
            tools
                .iter()
                .map(|tool| tool["id"].as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// `features` 里归包 `package` 的功能：`(编号, 工具名)`，工具照目录的先后。
fn flat(result: &Value, package: &str) -> Vec<(String, Vec<String>)> {
    result["features"]
        .as_array()
        .expect("有 features")
        .iter()
        .filter(|one| one["package"] == package)
        .map(|one| {
            let tools = one["tools"]
                .as_array()
                .expect("有 tools")
                .iter()
                .map(|tool| tool["name"].as_str().unwrap_or_default().to_string())
                .collect();
            (one["id"].as_str().unwrap_or_default().to_string(), tools)
        })
        .collect()
}

/// 接平台的包：全是私聊、群的那一项不列，混着的只列带本机的两件，全是本机的照列。
#[tokio::test]
async fn a_platform_package_lists_only_what_local_sessions_get() {
    let home = Home::new();
    let program = Program::new();
    let mut client = connected(&home, &program).await;
    let full = get(&mut client, "full").await;
    let bridge = section(&full, "bridge");
    assert_eq!(
        rows(bridge),
        [
            ("mixed".into(), "聊天动作".into(), true, "default".into()),
            ("home".into(), "本机".into(), true, "default".into()),
        ],
        "{bridge}"
    );
    assert_eq!((&bridge["on"], &bridge["total"]), (&json!(2), &json!(2)));
    assert_eq!(tool_ids(&full, "mixed"), ["qq_send", "qq_poke"]);
    assert_eq!(tool_ids(&full, "home"), ["note_a", "note_b"]);
    assert_eq!(
        flat(&full, "bridge"),
        [
            (
                "mixed".to_string(),
                vec!["qq_poke".to_string(), "qq_send".to_string()]
            ),
            (
                "home".to_string(),
                vec!["note_a".to_string(), "note_b".to_string()]
            ),
        ],
        "平铺的 features 也一样（照目录的先后）"
    );
}

/// 不接平台的包照旧：工具只给私聊的也全列。
#[tokio::test]
async fn a_package_without_connection_lists_everything() {
    let home = Home::new();
    let program = Program::new();
    let mut client = connected(&home, &program).await;
    let full = get(&mut client, "full").await;
    assert_eq!(
        rows(section(&full, "extensions")),
        [("chat".into(), "plain".into(), true, "default".into())]
    );
    assert_eq!(tool_ids(&full, "chat"), ["plain_say", "plain_ask"]);
    assert_eq!(
        flat(&full, "plain"),
        [(
            "chat".to_string(),
            vec!["plain_ask".to_string(), "plain_say".to_string()]
        )]
    );
}

/// 预设里写着藏掉的功能、工具的开关：照常读得出，列出来的照自己写的，藏掉的不另列成「没装」。
#[tokio::test]
async fn switches_for_hidden_entries_are_harmless() {
    let home = Home::new();
    let program = Program::new();
    home.write(
        "root/presets/p.toml",
        "[preset]\nname = { en = \"P\" }\n\n[features]\nremote = false\nhome = false\n\n[tools]\nqq_kick = false\nqq_history = false\nqq_poke = false\n",
    );
    let mut client = connected(&home, &program).await;
    let got = get(&mut client, "p").await;
    assert_eq!(
        rows(section(&got, "bridge")),
        [
            ("mixed".into(), "聊天动作".into(), true, "default".into()),
            ("home".into(), "本机".into(), false, "preset".into()),
        ],
        "{got}"
    );
    let poke = section(&got, "bridge")["items"][0]["tools"][1].clone();
    assert_eq!(
        (&poke["id"], &poke["on"]),
        (&json!("qq_poke"), &json!(false))
    );
    let listed: Vec<&Value> = got["features"]
        .as_array()
        .expect("有 features")
        .iter()
        .filter(|one| one["id"] == "remote")
        .collect();
    assert!(listed.is_empty(), "藏掉的不另列成没装的：{listed:?}");
}
