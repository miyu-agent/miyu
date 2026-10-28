//! 脚本登记进工具目录（施工 X-12 中，`docs/blueprint/skills.md`「脚本」）：真核心走一遍。核心起来时把管理员家目录里放的脚本
//! 登记成工具，会话的工具面里有它；她调它照跑命令的规矩（这个核心没有能用的沙盒、没人能确认：拒绝，真跑在基础系统那边测）；每个脚本一个功能 `script-<名字>`，`preset.get` 列出来，预设关掉的会话里没有；
//! 撞了别的工具名字的不登记；装上一个带脚本的包，当场登记上；`check` 报跳过的脚本、技能和撞名没登记上的脚本（施工 X-12 下），
//! 链接指到数据根里不认的地方的脚本、技能、斜杠模板（施工 X-37）。
//! 时间线上写开头的显示名（施工 9-8 六补）。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::builtins::Builtins;
use miyu_kernel::event::Body;
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_store::packages::Found;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::venues::configured_core;
use crate::support::*;

mod check;
mod clicall;
mod follow;
mod loading;

/// 造不成工具的脚本名：[`Kit`] 遇到它交回错（施工 X-46）。
const UNMADE: &str = "unmade";

/// 只会造脚本工具的端口：内置包的工具不动。
struct Kit;

impl Builtins for Kit {
    fn tools(&self, _: &[Found]) -> Result<Vec<miyu_endpoint::builtins::Group>, String> {
        Ok(Vec::new())
    }

    fn settings(&self, found: &mut [Found]) -> Vec<miyu_config::Item> {
        let own = [
            miyu_endpoint::settings::UiSettings::ITEMS,
            miyu_endpoint::settings::PersonaSettings::ITEMS,
            miyu_endpoint::settings::PresetSettings::ITEMS,
            miyu_endpoint::settings::PermissionSettings::ITEMS,
            miyu_endpoint::settings::EXTERNAL_BINDINGS,
            miyu_session::loading::ITEMS,
            miyu_session::switches::ITEMS,
            miyu_session::packages_off::ITEMS,
        ]
        .concat();
        let packaged = miyu_endpoint::packages::settle(found, &own);
        own.into_iter().chain(packaged).collect()
    }

    fn embed(&self, _: &[&Found]) -> Option<miyu_session::EmbedSetup> {
        None
    }

    fn script(
        &self,
        head: &miyu_policy::script::ScriptHead,
        file: std::path::PathBuf,
        source: &str,
    ) -> Result<Arc<dyn Tool>, String> {
        if head.name == UNMADE {
            return Err(format!("Cannot make {UNMADE}."));
        }
        miyu_basesystem::script(&default_resources(), head, file, source)
            .map_err(|error| error.to_string())
    }
}

/// 一个 Python 脚本：把收到的参数原样印出来。
pub(crate) fn echo(name: &str) -> String {
    format!(
        "#!/usr/bin/env python3\n# 显示名称：回声\n# Description: Echo {name}.\n# 描述：原样印出参数。\n# Need load: no\n# Parameters: {{\"type\": \"object\", \"properties\": {{\"say\": {{\"type\": \"string\"}}}}}}\nimport os\nprint('{name}:' + os.environ['MIYU_ARGS_JSON'])\n"
    )
}

/// 读系统配置、个人设置的核心，工具目录里有基础系统的 `read`，装着造脚本的端口；照起来时那样登记脚本。
pub(crate) fn core(home: &Home, script: &Script) -> Arc<Core> {
    let read: Arc<dyn Tool> = Fake::new("read", Access::Read, Act::Echo);
    // 施工 BS-1 上中起 read 是自己一个包。
    let core = configured_core(
        home,
        script,
        Catalog::in_packages([("read", vec![read])]).expect("合写法"),
    );
    let core = Arc::new(
        Arc::into_inner(core)
            .expect("只有这一份")
            .with_builtins(Arc::new(Kit)),
    );
    core.start_extensions();
    core
}

/// 照 `params` 造一个会话、说一句，交回会话编号。
pub(crate) async fn session(client: &mut Client, home: &Home, id: &str, params: Value) -> String {
    let reply = client.call(id, "session.create", params).await;
    let session = reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("没造出来：{reply}"))
        .to_string();
    client.say(&format!("{id}-say"), &session, "hi").await;
    home.until_turns(&session, 1).await;
    session
}

pub(crate) fn feature<'a>(reply: &'a Value, id: &str) -> Option<&'a Value> {
    reply["result"]["features"]
        .as_array()
        .expect("是一个个功能")
        .iter()
        .find(|one| one["id"] == id)
}

#[tokio::test]
async fn a_home_script_becomes_a_tool_that_runs_like_a_command() {
    let home = Home::new();
    home.write("scripts/weather.py", &echo("weather"));
    // 撞了基础系统的 `read`：不登记。
    home.write("scripts/read.py", &echo("read"));
    let script = Script::new([
        Play::calls(&[("weather", r#"{"say":"晴"}"#)]),
        Play::Says("好。"),
    ]);
    let core = core(&home, &script);
    let names: Vec<String> = core
        .scripts()
        .iter()
        .map(|one| one.head.name.clone())
        .collect();
    assert_eq!(names, ["weather"], "撞名的不登记");
    let mut client = Client::connect(core);
    client.hello_without_input().await;
    let work = home.work.to_string_lossy().into_owned();
    let session = session(
        &mut client,
        &home,
        "c1",
        json!({"cwd": work, "preset": "full"}),
    )
    .await;
    let tools: Vec<String> = script.requests()[0]
        .1
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect();
    assert_eq!(tools, ["read", "weather"]);
    let result = home
        .log(&session)
        .into_iter()
        .find_map(|event| match event.body {
            Body::ToolResult(result) => Some(format!("{:?}", result.blocks)),
            _ => None,
        })
        .expect("有结果");
    assert!(
        result.contains("needs the user's approval, which no one can give here"),
        "访问类别是执行：没有沙盒、没人能确认的拒绝：{result}"
    );
    // 时间线上写开头的显示名（施工 9-8 六补）：内核替它写的拒绝不记，照这一刻工具目录里的。
    let reply = client
        .call("p1", "view.page", json!({"session": session, "view": true}))
        .await;
    let titles: Vec<&str> = reply["result"]["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|entry| entry["kind"] == "tool")
        .filter_map(|entry| entry["title"]["name"].as_str())
        .collect();
    assert_eq!(titles, ["回声"], "{reply}");
}

#[tokio::test]
async fn each_script_is_a_feature_the_preset_switches() {
    let home = Home::new();
    home.write("scripts/weather.py", &echo("weather"));
    home.write(
        "presets/lean.toml",
        "[preset]\nname = { en = \"Lean\" }\nunlisted = \"on\"\n\n[features]\nscript-weather = false\n",
    );
    let script = Script::new([Play::Says("一。"), Play::Says("二。")]);
    let mut client = Client::connect(core(&home, &script));
    client.hello().await;
    let full = client
        .call("g1", "preset.get", json!({"preset": "full"}))
        .await;
    assert_eq!(
        feature(&full, "script-weather"),
        Some(&json!({
            "id": "script-weather",
            "name": "回声",
            "summary": "原样印出参数。",
            "package": "~home",
            "on": true,
            "by": "default",
            "installed": true,
            "tools": [{"name": "weather", "label": "回声", "on": true}],
        }))
    );
    let lean = client
        .call("g2", "preset.get", json!({"preset": "lean"}))
        .await;
    assert_eq!(
        feature(&lean, "script-weather").map(|one| &one["on"]),
        Some(&json!(false))
    );
    let work = home.work.to_string_lossy().into_owned();
    session(
        &mut client,
        &home,
        "c1",
        json!({"cwd": work, "preset": "lean"}),
    )
    .await;
    let tools: Vec<String> = script.requests()[0]
        .1
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect();
    assert_eq!(tools, ["read"], "预设关掉的没有");
}

#[tokio::test]
async fn a_package_with_scripts_registers_them_when_installed() {
    let home = Home::new();
    let folder = home.work.join("kit");
    std::fs::create_dir_all(folder.join("scripts")).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        "[package]\nprotocol = [1, 1]\nname = { zh = \"脚本包\" }\n\n[command]\nname = \"kit\"\nprogram = \"miyu-kit-absent\"\nabout = { en = \"K\" }\n\n[process]\n",
    )
    .expect("写得进");
    std::fs::write(folder.join("scripts/bangumi.py"), echo("bangumi")).expect("写得进");
    home.write(
        "presets/nokit.toml",
        "[preset]\nname = { en = \"No kit\" }\nunlisted = \"on\"\n\n[software]\nkit = false\n",
    );
    let script = Script::new([Play::Says("一。"), Play::Says("二。")]);
    let core = core(&home, &script);
    assert!(core.scripts().is_empty());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let installed = client
        .call("i1", "package.install", json!({"path": folder}))
        .await;
    assert_eq!(installed["result"]["package"], "kit", "{installed}");
    let scripts = core.scripts();
    assert_eq!(scripts.len(), 1);
    assert_eq!(scripts[0].package.as_deref(), Some("kit"));
    let work = home.work.to_string_lossy().into_owned();
    session(
        &mut client,
        &home,
        "c1",
        json!({"cwd": work, "preset": "full"}),
    )
    .await;
    session(
        &mut client,
        &home,
        "c2",
        json!({"cwd": work, "preset": "nokit"}),
    )
    .await;
    let faces: Vec<Vec<String>> = script
        .requests()
        .iter()
        .map(|(_, request)| request.tools.iter().map(|tool| tool.name.clone()).collect())
        .collect();
    assert_eq!(faces[0], ["bangumi", "read"], "装上当场登记上");
    assert_eq!(
        faces[1],
        ["read"],
        "挂在包 kit 下：整个包关掉的，脚本跟着关"
    );
}

/// 只带技能、脚本的包（施工 X-13 上）：清单只写 `[package]`；装之前的预览列出它带的技能、脚本；装上以后没有开关（状态是
/// 就绪），技能列进新会话的 system、脚本登记成工具。
#[tokio::test]
async fn a_package_with_only_skills_and_scripts_installs_without_a_switch() {
    let home = Home::new();
    let folder = home.work.join("notes");
    std::fs::create_dir_all(folder.join("scripts")).expect("建得了");
    std::fs::create_dir_all(folder.join("skills/take-notes")).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        "[package]\nname = { zh = \"笔记\" }\n",
    )
    .expect("写得进");
    std::fs::write(folder.join("scripts/dice.py"), echo("dice")).expect("写得进");
    std::fs::write(
        folder.join("skills/take-notes/SKILL.md"),
        "---\nname: take-notes\ndescription: Take notes.\n---\nbody\n",
    )
    .expect("写得进");
    // 没有说明的读不成（施工 X-13 补；原来用的「说明里不加引号的冒号」施工 PT-2 上起照宽松读法照收）。
    std::fs::create_dir_all(folder.join("skills/bad")).expect("建得了");
    std::fs::write(
        folder.join("skills/bad/SKILL.md"),
        "---\nname: bad\n---\nbody\n",
    )
    .expect("写得进");
    let script = Script::new([Play::Says("一。")]);
    let core = core(&home, &script);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let shown = client
        .call(
            "p1",
            "package.install",
            json!({"path": folder, "preview": true}),
        )
        .await;
    let result = &shown["result"];
    assert_eq!(
        (
            result["skills"].clone(),
            result["scripts"].clone(),
            result.get("program").cloned()
        ),
        (json!(["take-notes"]), json!(["dice"]), None),
        "{shown}"
    );
    let skipped = result["skipped"]
        .as_array()
        .unwrap_or_else(|| panic!("{shown}"));
    assert_eq!(skipped.len(), 1, "{shown}");
    assert_eq!(skipped[0]["file"], "skills/bad/SKILL.md", "{shown}");
    assert!(
        skipped[0]["why"]
            .as_str()
            .is_some_and(|why| !why.is_empty())
    );
    let installed = client
        .call("i1", "package.install", json!({"path": folder}))
        .await;
    assert_eq!(installed["result"]["package"], "notes", "{installed}");
    let listed = client.call("l1", "package.list", json!({})).await;
    let entry = listed["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"))
        .iter()
        .find(|one| one["package"] == "notes")
        .cloned()
        .unwrap_or_else(|| panic!("{listed}"));
    assert_eq!(entry["status"], "ready", "{entry}");
    assert!(entry.get("enabled").is_none(), "没有开关：{entry}");
    let switched = client
        .call("e1", "package.enable", json!({"package": "notes"}))
        .await;
    assert_eq!(
        switched["error"]["data"]["reason"], "not_switchable",
        "{switched}"
    );
    let restarted = client
        .call("r1", "package.restart", json!({"package": "notes"}))
        .await;
    assert_eq!(
        restarted["error"]["data"]["reason"], "not_restartable",
        "{restarted}"
    );
    let work = home.work.to_string_lossy().into_owned();
    session(
        &mut client,
        &home,
        "c1",
        json!({"cwd": work, "preset": "full"}),
    )
    .await;
    let (_, request) = &script.requests()[0];
    let tools: Vec<&str> = request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect();
    assert_eq!(tools, ["dice", "read"]);
    assert!(
        request.system.contains("- take-notes: Take notes."),
        "{}",
        request.system
    );
}

/// 同名被盖住的（施工 X-13 中，设计 `31-软件包.md` 第五节第 3 条）：家目录里放的赢，装的包里同名的那一份 `check` 报
/// `script_covered`、`skill_covered`，说被谁盖住。
#[tokio::test]
async fn check_says_which_file_covers_a_packaged_script_or_skill() {
    let home = Home::new();
    home.write("scripts/dice.py", &echo("dice"));
    home.write(
        "skills/notes/SKILL.md",
        "---\nname: notes\ndescription: Mine.\n---\nbody\n",
    );
    let folder = home.work.join("kit");
    std::fs::create_dir_all(folder.join("scripts")).expect("建得了");
    std::fs::create_dir_all(folder.join("skills/notes")).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        "[package]\nname = { en = \"Kit\" }\n",
    )
    .expect("写得进");
    std::fs::write(folder.join("scripts/dice.py"), echo("dice")).expect("写得进");
    std::fs::write(
        folder.join("skills/notes/SKILL.md"),
        "---\nname: notes\ndescription: Theirs.\n---\nbody\n",
    )
    .expect("写得进");
    let mut client = Client::connect(core(&home, &Script::new([])));
    client.hello().await;
    let installed = client
        .call("i1", "package.install", json!({"path": folder}))
        .await;
    assert_eq!(installed["result"]["package"], "kit", "{installed}");
    let reply = client.call("k1", "check", json!({})).await;
    let problems: Vec<(String, String, String)> = reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .map(|one| {
            let text = |key: &str| one[key].as_str().unwrap_or_default().to_string();
            (text("code"), text("file"), text("message"))
        })
        .collect();
    assert_eq!(problems.len(), 2, "{reply}");
    let (code, file, message) = &problems[0];
    assert_eq!(code, "script_covered");
    assert!(file.ends_with("kit/scripts/dice.py"), "{file}");
    assert!(message.contains("scripts/dice.py"), "{message}");
    let (code, file, message) = &problems[1];
    assert_eq!(code, "skill_covered");
    assert!(file.ends_with("kit/skills/notes"), "{file}");
    assert!(message.contains("skills/notes/SKILL.md"), "{message}");
}
