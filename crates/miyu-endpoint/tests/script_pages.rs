//! 技能、脚本、MCP 三页要的协议（施工 X-27，`docs/blueprint/skills.md`「脚本」、`presets.md`「全局开关」）：`script.list`
//! 列出登记上的脚本，给人看的两格照连接的语言挑；`tool.set_loading` 改加载方式，`need_load.by` 跟着从 `source` 变成
//! `setting`、下一个会话的工具面照新的；`feature.set` 改全局开关，没写这个功能的预设照它、明写了的照预设，`preset.get`
//! 每一项说是谁定的；写错的拒，什么都不写。

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};

use crate::scripts::{core, echo, feature, session};
use crate::support::*;

/// 个人设置的字。
fn settings(home: &Home) -> String {
    std::fs::read_to_string(home.root.path().join("home/alice/settings.toml")).unwrap_or_default()
}

/// 拒绝的原因码。
fn reason(reply: &Value) -> &str {
    reply["error"]["data"]["reason"]
        .as_str()
        .unwrap_or_default()
}

/// `script.list` 里工具名是 `tool` 的那一项。
async fn listed(client: &mut Client, tool: &str) -> Value {
    let reply = client.call("l", "script.list", json!({})).await;
    reply["result"]["scripts"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .find(|one| one["tool"] == tool)
        .cloned()
        .unwrap_or_else(|| panic!("没有 {tool}：{reply}"))
}

/// 第 `n` 次请求的工具面：名字和参数。
fn face(script: &Script, n: usize) -> Vec<(String, String)> {
    script.requests()[n]
        .1
        .tools
        .iter()
        .map(|tool| (tool.name.clone(), tool.parameters.get().to_string()))
        .collect()
}

/// 会话 `session` 的日志里有没有换过快照（`session.policy_changed` 带新策略的）。
fn swapped(home: &Home, session: &str) -> bool {
    home.log(session).iter().any(
        |event| matches!(&event.body, Body::PolicyChanged(changed) if changed.policy.is_some()),
    )
}

/// 工具面上的名字。
fn names(face: &[(String, String)]) -> Vec<&str> {
    face.iter().map(|(name, _)| name.as_str()).collect()
}

/// 写好三个脚本：家目录里的 `weather`（全量、等 5 秒）、`plain`（什么给人看的都没写、先加载），装的包 `kit` 带的 `dice`；
/// 个人设置的语言是 `language`。
fn written(home: &Home, language: &str) {
    home.write(
        "home/alice/settings.toml",
        &format!("[ui]\nlanguage = \"{language}\"\n"),
    );
    let weather = echo("weather").replace(
        "# 显示名称：回声\n",
        "# Display name: Echo\n# 显示名称：回声\n# Timeout: 5\n",
    );
    home.write("home/alice/scripts/weather.py", &weather);
    home.write(
        "home/alice/scripts/plain.py",
        "#!/usr/bin/env python3\n# Description: Plain.\nprint('plain')\n",
    );
    home.write(
        "system/packages/kit/package.toml",
        "[package]\nname = { en = \"kit\" }\n",
    );
    home.write("system/packages/kit/scripts/dice.py", &echo("dice"));
}

#[tokio::test]
async fn scripts_are_listed_with_where_they_come_from_in_the_connection_language() {
    for (language, display, description) in [
        ("zh", "回声", "原样印出参数。"),
        ("en", "Echo", "原样印出参数。"),
    ] {
        let home = Home::new();
        written(&home, language);
        let mut client = Client::connect(core(&home, &Script::new([])));
        client.hello().await;
        let reply = client.call("l", "script.list", json!({})).await;
        let scripts = reply["result"]["scripts"]
            .as_array()
            .unwrap_or_else(|| panic!("{reply}"));
        let tools: Vec<&str> = scripts
            .iter()
            .map(|one| one["tool"].as_str().unwrap_or_default())
            .collect();
        assert_eq!(tools, ["dice", "plain", "weather"], "照工具名排");
        let weather = &scripts[2];
        let path = weather["path"]
            .as_str()
            .unwrap_or_default()
            .replace('\\', "/");
        assert!(path.ends_with("home/alice/scripts/weather.py"), "{path}");
        let mut rest = weather.clone();
        rest.as_object_mut().expect("是一张表").remove("path");
        assert_eq!(
            rest,
            json!({
                "tool": "weather",
                "display_name": display,
                "description": description,
                "from": "home",
                "timeout": 5,
                "need_load": {"value": false, "by": "source"},
                "on": true,
            }),
            "{language}"
        );
        let plain = &scripts[1];
        assert_eq!(plain["timeout"], 60, "没写的 60 秒");
        assert_eq!(plain["need_load"], json!({"value": true, "by": "source"}));
        assert!(plain.get("display_name").is_none() && plain.get("description").is_none());
        let dice = &scripts[0];
        assert_eq!(
            (&dice["from"], &dice["package"]),
            (&json!("package"), &json!("kit"))
        );
        let path = dice["path"].as_str().unwrap_or_default().replace('\\', "/");
        assert!(path.ends_with("kit/scripts/dice.py"), "{path}");
    }
}

#[tokio::test]
async fn set_loading_moves_the_source_to_the_setting_and_the_next_face() {
    let home = Home::new();
    home.write(
        "home/alice/scripts/weather.py",
        &echo("weather").replace("# Need load: no\n", ""),
    );
    home.write(
        "home/alice/settings.toml",
        "[ui]\nlanguage = \"zh\"\n\n[mcp.github]\ncommand = [\"miyu-absent-mcp-server\"]\n",
    );
    let script = Script::new([Play::Says("一。"), Play::Says("二。"), Play::Says("三。")]);
    let mut client = Client::connect(core(&home, &script));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let open = json!({"cwd": work, "preset": "full"});
    assert_eq!(
        listed(&mut client, "weather").await["need_load"],
        json!({"value": true, "by": "source"})
    );
    session(&mut client, &home, "c1", open.clone()).await;
    assert_eq!(names(&face(&script, 0)), ["load_tools", "read", "weather"]);

    let set = client
        .call(
            "t1",
            "tool.set_loading",
            json!({"tool": "weather", "need_load": false}),
        )
        .await;
    assert_eq!(set["result"], json!({}), "{set}");
    assert!(
        settings(&home).contains("weather = false"),
        "{}",
        settings(&home)
    );
    assert_eq!(
        listed(&mut client, "weather").await["need_load"],
        json!({"value": false, "by": "setting"})
    );
    session(&mut client, &home, "c2", open.clone()).await;
    let second = face(&script, 1);
    assert_eq!(names(&second), ["read", "weather"], "人改成全量的照全量");
    assert!(
        second[1].1.contains("\"say\""),
        "完整的参数：{}",
        second[1].1
    );

    // 删掉那一项：回到来处。
    let unset = client
        .call(
            "t2",
            "tool.set_loading",
            json!({"tool": "weather", "need_load": null}),
        )
        .await;
    assert_eq!(unset["result"], json!({}), "{unset}");
    assert!(!settings(&home).contains("weather"), "{}", settings(&home));
    assert_eq!(
        listed(&mut client, "weather").await["need_load"],
        json!({"value": true, "by": "source"})
    );
    session(&mut client, &home, "c3", open).await;
    assert_eq!(names(&face(&script, 2)), ["load_tools", "read", "weather"]);

    // MCP 服务那一级：`mcp.list` 照 `loading.mcp.<服务名>` 说。
    let server = |reply: &Value| reply["result"]["servers"][0].clone();
    let before = client.call("m1", "mcp.list", json!({})).await;
    assert_eq!(
        (&server(&before)["need_load"], &server(&before)["on"]),
        (&json!({"value": true, "by": "source"}), &json!(true)),
        "{before}"
    );
    let set = client
        .call(
            "t3",
            "tool.set_loading",
            json!({"server": "github", "need_load": false}),
        )
        .await;
    assert_eq!(set["result"], json!({}), "{set}");
    let after = client.call("m2", "mcp.list", json!({})).await;
    assert_eq!(
        server(&after)["need_load"],
        json!({"value": false, "by": "setting"}),
        "{after}"
    );
    // 全局开关：`mcp.list` 的 `on` 照 `features.mcp-<服务名>`。
    let off = client
        .call(
            "f1",
            "feature.set",
            json!({"feature": "mcp-github", "on": false}),
        )
        .await;
    assert_eq!(off["result"], json!({}), "{off}");
    let after = client.call("m3", "mcp.list", json!({})).await;
    assert_eq!(server(&after)["on"], false, "{after}");
}

#[tokio::test]
async fn a_global_switch_rules_where_the_preset_says_nothing() {
    let home = Home::new();
    home.write("home/alice/scripts/weather.py", &echo("weather"));
    home.write(
        "home/alice/skills/notes/SKILL.md",
        "---\nname: notes\ndescription: Take notes.\n---\n\nBody.\n",
    );
    home.write(
        "home/alice/presets/keep.toml",
        "[preset]\nname = { en = \"Keep\" }\nunlisted = \"on\"\n\n[features]\nscript-weather = true\n",
    );
    let says = ["一。", "二。", "三。", "四。", "五。"];
    let script = Script::new(says.map(Play::Says));
    let mut client = Client::connect(core(&home, &script));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let open = session(
        &mut client,
        &home,
        "c0",
        json!({"cwd": work, "preset": "full"}),
    )
    .await;
    assert_eq!(names(&face(&script, 0)), ["read", "weather"]);
    let decided = |reply: &Value| {
        let one = feature(reply, "script-weather").expect("列了");
        (one["on"].clone(), one["by"].clone())
    };
    let full = client
        .call("g1", "preset.get", json!({"preset": "full"}))
        .await;
    assert_eq!(decided(&full), (json!(true), json!("default")), "都没写");
    let keep = client
        .call("g2", "preset.get", json!({"preset": "keep"}))
        .await;
    assert_eq!(decided(&keep), (json!(true), json!("preset")), "预设里写的");

    let set = client
        .call(
            "f1",
            "feature.set",
            json!({"feature": "script-weather", "on": false}),
        )
        .await;
    assert_eq!(set["result"], json!({}), "{set}");
    assert!(
        settings(&home).contains("script-weather = false"),
        "{}",
        settings(&home)
    );
    assert_eq!(listed(&mut client, "weather").await["on"], false);
    // 技能的全局开关：`skill.list` 的 `on`。
    client
        .call(
            "f3",
            "feature.set",
            json!({"feature": "skill-notes", "on": false}),
        )
        .await;
    let skills = client.call("s1", "skill.list", json!({})).await;
    let notes = skills["result"]["skills"]
        .as_array()
        .and_then(|all| all.iter().find(|one| one["name"] == "notes"))
        .unwrap_or_else(|| panic!("{skills}"));
    assert_eq!(notes["on"], false, "{skills}");
    let full = client
        .call("g3", "preset.get", json!({"preset": "full"}))
        .await;
    assert_eq!(
        decided(&full),
        (json!(false), json!("global")),
        "照全局开关"
    );
    let keep = client
        .call("g4", "preset.get", json!({"preset": "keep"}))
        .await;
    assert_eq!(
        decided(&keep),
        (json!(true), json!("preset")),
        "预设明写的照预设"
    );

    // 开着的会话下一个回合换上，新开的会话照新的。
    client.say("c0-again", &open, "再说").await;
    home.until_turns(&open, 2).await;
    assert_eq!(
        names(&face(&script, 1)),
        ["read"],
        "开着的会话下一个回合没有了"
    );
    assert!(swapped(&home, &open), "开着的会话换了一次快照");
    let fresh = session(
        &mut client,
        &home,
        "c1",
        json!({"cwd": work, "preset": "full"}),
    )
    .await;
    assert!(
        !swapped(&home, &fresh),
        "新会话造的时候就照全局开关，第一轮不用再换快照"
    );
    session(
        &mut client,
        &home,
        "c2",
        json!({"cwd": work, "preset": "keep"}),
    )
    .await;
    assert_eq!(names(&face(&script, 2)), ["read"], "全局关掉的没有");
    assert_eq!(
        names(&face(&script, 3)),
        ["read", "weather"],
        "预设里明写开的照样有"
    );

    let unset = client
        .call(
            "f2",
            "feature.set",
            json!({"feature": "script-weather", "on": null}),
        )
        .await;
    assert_eq!(unset["result"], json!({}), "{unset}");
    assert_eq!(listed(&mut client, "weather").await["on"], true);
    session(
        &mut client,
        &home,
        "c3",
        json!({"cwd": work, "preset": "full"}),
    )
    .await;
    assert_eq!(
        names(&face(&script, 4)),
        ["read", "weather"],
        "删掉以后回到默认"
    );
}

/// 全局开关只管默认开的预设（2026-10-20 项目主人定）：打开的在 `unlisted = "off"` 的预设里照旧没有、`by` 是 `default`；
/// 同一个功能在默认开的预设里，全局关掉就没有。
#[tokio::test]
async fn a_global_switch_leaves_an_off_preset_alone() {
    let home = Home::new();
    home.write("home/alice/scripts/weather.py", &echo("weather"));
    home.write(
        "home/alice/presets/bare.toml",
        "[preset]\nname = { en = \"Bare\" }\nunlisted = \"off\"\n",
    );
    let script = Script::new([Play::Says("一。"), Play::Says("二。")]);
    let mut client = Client::connect(core(&home, &script));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let on = client
        .call(
            "f1",
            "feature.set",
            json!({"feature": "script-weather", "on": true}),
        )
        .await;
    assert_eq!(on["result"], json!({}), "{on}");
    let bare = client
        .call("g1", "preset.get", json!({"preset": "bare"}))
        .await;
    let weather = feature(&bare, "script-weather").expect("列了");
    assert_eq!(
        (&weather["on"], &weather["by"]),
        (&json!(false), &json!("default")),
        "{bare}"
    );
    session(
        &mut client,
        &home,
        "c1",
        json!({"cwd": work, "preset": "bare"}),
    )
    .await;
    assert!(
        !names(&face(&script, 0)).contains(&"weather"),
        "默认关的预设不照全局开关"
    );
    let off = client
        .call(
            "f2",
            "feature.set",
            json!({"feature": "script-weather", "on": false}),
        )
        .await;
    assert_eq!(off["result"], json!({}), "{off}");
    session(
        &mut client,
        &home,
        "c2",
        json!({"cwd": work, "preset": "full"}),
    )
    .await;
    assert_eq!(names(&face(&script, 1)), ["read"], "默认开的预设照全局关掉");
}

#[tokio::test]
async fn badly_written_changes_are_refused_and_nothing_is_written() {
    let home = Home::new();
    let before = "[ui]\nlanguage = \"zh\"\n";
    home.write("home/alice/settings.toml", before);
    let mut client = Client::connect(core(&home, &Script::new([])));
    client.hello().await;
    let long = "a".repeat(129);
    for (method, params) in [
        ("tool.set_loading", json!({"need_load": true})),
        (
            "tool.set_loading",
            json!({"tool": "weather", "server": "github", "need_load": true}),
        ),
        ("tool.set_loading", json!({"tool": "weather"})),
        (
            "tool.set_loading",
            json!({"tool": "a.b", "need_load": true}),
        ),
        ("tool.set_loading", json!({"tool": long, "need_load": true})),
        (
            "tool.set_loading",
            json!({"server": "GitHub", "need_load": true}),
        ),
        (
            "tool.set_loading",
            json!({"tool": "weather", "need_load": "yes"}),
        ),
        ("feature.set", json!({"feature": "memory", "on": false})),
        ("feature.set", json!({"feature": "skill-a.b", "on": false})),
        ("feature.set", json!({"feature": "skill-a"})),
        (
            "feature.set",
            json!({"feature": "skill-a", "on": false, "x": 1}),
        ),
    ] {
        let reply = client.call("x", method, params.clone()).await;
        assert_eq!(reason(&reply), "bad_params", "{method} {params}：{reply}");
    }
    assert_eq!(settings(&home), before, "拒了的什么都不写");
}
