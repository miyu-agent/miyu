//! 技能照预设开关（施工 X-11 上，`docs/blueprint/skills.md`「定了的」）：每个技能算一个功能 `skill-<名字>`。`preset.get` 列出
//! 它（名字、说明照技能写给人看的两格），基础功能（没列出来的关着）里关着、全部功能里开着、写了关的关着，写过的不再当没装的
//! 另列一遍；照预设造的会话只列开着的、只放开开着的文件夹，换一份核心像重启过，载入的会话照样只放开开着的。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, ToolStatus};
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::venues::configured_core;
use crate::support::*;

/// 在 alice 家目录里放两个技能：`notes` 写了给人看的名字、说明，`plain` 没写。交回 `notes` 的 `SKILL.md` 的路径。
fn put(home: &Home) -> String {
    home.write(
        "home/alice/skills/notes/SKILL.md",
        "---\nname: notes\ndescription: Take notes.\nmetadata:\n  display_name: 笔记\n  summary: 照格式记笔记\n---\n照这样记。\n",
    );
    home.write(
        "home/alice/skills/plain/SKILL.md",
        "---\nname: plain\ndescription: Plain one.\n---\n正文\n",
    );
    home.write(
        "home/alice/presets/lean.toml",
        "[preset]\nname = { en = \"Lean\" }\nunlisted = \"on\"\n\n[features]\nskill-notes = false\n",
    );
    home.root
        .account_dir(&alice())
        .join("skills")
        .join("notes")
        .join("SKILL.md")
        .to_string_lossy()
        .into_owned()
}

fn feature<'a>(reply: &'a Value, id: &str) -> &'a Value {
    reply["result"]["features"]
        .as_array()
        .expect("是一个个功能")
        .iter()
        .find(|one| one["id"] == id)
        .unwrap_or_else(|| panic!("没有 {id}：{reply}"))
}

#[tokio::test]
async fn preset_get_lists_each_skill_as_a_feature() {
    let home = Home::new();
    put(&home);
    let mut client = Client::connect(configured_core(&home, &Script::new([]), Catalog::default()));
    client.hello().await;
    let get = |id: &'static str| json!({ "preset": id });
    let full = client.call("g1", "preset.get", get("full")).await;
    assert_eq!(
        feature(&full, "skill-notes"),
        &json!({"id": "skill-notes", "name": "笔记", "summary": "照格式记笔记", "on": true, "installed": true, "tools": []})
    );
    assert_eq!(
        feature(&full, "skill-plain")["name"],
        "plain",
        "没写名字的照技能的名字"
    );
    assert!(feature(&full, "skill-plain").get("summary").is_none());
    let dev = client.call("g2", "preset.get", get("dev")).await;
    assert_eq!(
        feature(&dev, "skill-notes")["on"],
        false,
        "基础功能：没列出来的关着"
    );
    let lean = client.call("g3", "preset.get", get("lean")).await;
    assert_eq!(feature(&lean, "skill-notes")["on"], false, "写了关的关着");
    assert_eq!(feature(&lean, "skill-plain")["on"], true);
    let notes: Vec<&Value> = lean["result"]["features"]
        .as_array()
        .expect("是一个个功能")
        .iter()
        .filter(|one| one["id"] == "skill-notes")
        .collect();
    assert_eq!(notes.len(), 1, "写过的不再当没装的另列：{lean}");
}

#[tokio::test]
async fn a_session_lists_and_opens_only_the_skills_its_preset_turns_on() {
    let home = Home::new();
    let skill = put(&home);
    let skill = if cfg!(windows) {
        skill.replace('\\', "/")
    } else {
        skill
    };
    let read = Fake::new("read", Access::Read, Act::Echo);
    let tools = || Catalog::new([Arc::clone(&read) as Arc<dyn Tool>]).expect("合写法");
    let script = || {
        Script::new([
            Play::calls(&[("read", &json!({ "path": skill }).to_string())]),
            Play::Says("好。"),
        ])
    };
    let first = script();
    let mut client = Client::connect(configured_core(&home, &first, tools()));
    client.hello_without_input().await;
    let work = home.work.to_string_lossy().into_owned();
    let reply = client
        .call(
            "c1",
            "session.create",
            json!({"cwd": work, "preset": "lean"}),
        )
        .await;
    let session = reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("没造出来：{reply}"))
        .to_string();
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let system = &first.requests()[0].1.system;
    assert!(system.contains("- plain: Plain one."), "{system}");
    assert!(!system.contains("- notes:"), "关着的不列：{system}");
    assert!(read.calls().is_empty(), "关着的文件夹不放开");
    // 换一份核心，像重启过：载入的会话照样只放开开着的。
    let mut client = Client::connect(configured_core(&home, &script(), tools()));
    client.hello_without_input().await;
    client.say("s2", &session, "again").await;
    home.until_turns(&session, 2).await;
    assert!(read.calls().is_empty(), "载入以后也不放开");
    let denied = home
        .log(&session)
        .into_iter()
        .filter(|event| {
            matches!(&event.body, Body::ToolResult(result) if result.status == ToolStatus::Denied)
        })
        .count();
    assert_eq!(denied, 2);
}
