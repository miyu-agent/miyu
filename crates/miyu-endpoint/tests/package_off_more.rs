//! 每个人关掉某个包（施工 U23 末下）：它往 system 尾部放的那一段（`system.append`）关了以后下一轮就拿掉，它登记的工具不在工具面上；
//! 设置页（`config.schema`、`config.get`）不给他看那个包的配置项，那个包的关包开关照给。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

/// 等到 `path` 里记下的有 `n` 行。
async fn lines(path: &std::path::Path, n: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(path).lines().count() < n {
        assert!(tokio::time::Instant::now() < deadline, "等不到 {n} 行");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 把个人设置里的 `key` 写成 `value`。
async fn set(client: &mut Client, n: u8, key: &str, value: bool) -> Value {
    client
        .call(
            &format!("config-9f2c4e1a7b3d5f61-{n}"),
            "config.set",
            json!({"layer": "personal", "changes": [{"key": key, "value": value}]}),
        )
        .await
}

#[tokio::test]
async fn what_a_switched_off_extension_appends_goes_next_turn() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "alpha");
    let provide = format!(
        "ask:provide:{}",
        json!({"tools": [{"name": "alpha_tool", "description": "A test tool.",
                          "input_schema": {"type": "object"}, "access": "read", "venues": ["local"]}]})
    );
    let append = r#"ask:system.append:{"content":"<alpha>A</alpha>"}"#.to_string();
    let all = [
        step,
        "hello".to_string(),
        provide,
        append,
        "serve".to_string(),
    ];
    let all: Vec<&str> = all.iter().map(String::as_str).collect();
    let capabilities = ["tools", "context.system"];
    install_asking(
        &home,
        "alpha",
        &program.name(),
        "always",
        &steps(&all),
        &capabilities,
    );
    approve(&home, "alpha", &capabilities);
    let script = Script::new([Play::Says("一。"), Play::Says("二。")]).titles([Play::Says("测试")]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    lines(&path, 4).await;
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "一").await;
    home.until_turns(&session, 1).await;
    let reply = set(&mut client, 1, "packages.alpha.off", true).await;
    assert!(reply["result"].is_object(), "{reply}");
    client.say("s2", &session, "二").await;
    home.until_turns(&session, 2).await;
    let requests = script.requests();
    assert!(
        requests[0].1.system.ends_with("<alpha>A</alpha>"),
        "关之前带着：{}",
        requests[0].1.system
    );
    assert!(
        !requests[1].1.system.contains("<alpha>"),
        "关了以后下一轮拿掉：{}",
        requests[1].1.system
    );
    assert!(
        !requests[1]
            .1
            .tools
            .iter()
            .any(|tool| tool.name == "alpha_tool"),
        "它登记的工具不在工具面上"
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn a_switched_off_package_s_settings_are_not_shown_but_its_switch_is() {
    let home = Home::new();
    home.write(
        "system/packages/kit/package.toml",
        "[package]\nprotocol = [1, 1]\nname = { en = \"Kit\" }\n\n[ui]\n\n[settings.mode]\ntype = \"text\"\ndefault = \"x\"\nlayers = [\"personal\"]\nname = { en = \"Mode\" }\n",
    );
    let core = crate::scripts::core(&home, &Script::new([]));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    // 测试的核心照重读清单那一次拼包的项：装一个别的包。
    let folder = home.work.join("other");
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        "[package]\nname = { en = \"Other\" }\n",
    )
    .expect("写得进");
    let installed = client
        .call("i", "package.install", json!({"path": folder}))
        .await;
    assert_eq!(installed["result"]["package"], "other", "{installed}");
    let keys = |reply: &Value| -> Vec<String> {
        reply["result"]["items"]
            .as_array()
            .unwrap_or_else(|| panic!("{reply}"))
            .iter()
            .filter_map(|one| one["key"].as_str().map(str::to_string))
            .collect()
    };
    let listed = client.call("l", "package.list", json!({})).await;
    let kit = listed["result"]["packages"]
        .as_array()
        .and_then(|all| all.iter().find(|one| one["package"] == "kit"))
        .cloned()
        .unwrap_or_else(|| panic!("{listed}"));
    assert!(kit.get("problem").is_none(), "kit 读成了：{kit}");
    let before = keys(&client.call("s1", "config.schema", json!({})).await);
    assert!(
        before.contains(&"kit.mode".to_string()),
        "关之前有：{before:?}"
    );
    let reply = set(&mut client, 2, "packages.kit.off", true).await;
    assert!(reply["result"].is_object(), "{reply}");
    let after = keys(&client.call("s2", "config.schema", json!({})).await);
    assert!(
        !after.contains(&"kit.mode".to_string()),
        "关了不给看：{after:?}"
    );
    assert!(
        after.contains(&"packages.kit.off".to_string()),
        "关包的开关照给，好开回来"
    );
    let got = client.call("g", "config.get", json!({})).await;
    let shown = got["result"]["items"]
        .as_object()
        .unwrap_or_else(|| panic!("{got}"));
    assert!(!shown.contains_key("kit.mode"), "{got}");
}

/// 关了的包带的 MCP 服务交的说明（施工 U23 末下）：下一轮从 system 里拿掉。
#[tokio::test]
async fn a_switched_off_package_s_mcp_instructions_go_next_turn() {
    use crate::support::mcp::{bundle, enable, open, until_status};
    let home = Home::new();
    bundle(
        &home,
        "probe",
        "probe",
        &["instructions:Use echo.", "prompts"],
    );
    let script = Script::new([Play::Says("一。"), Play::Says("二。")]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    enable(&mut client, "probe").await;
    until_status(&mut client, "probe", "running").await;
    let session = open(&mut client).await;
    client.say("s1", &session, "一").await;
    home.until_turns(&session, 1).await;
    assert!(
        menu(&mut client).await.contains(&"probe:hello".to_string()),
        "关之前菜单里有它的提示词"
    );
    let reply = set(&mut client, 3, "packages.probe.off", true).await;
    assert!(reply["result"].is_object(), "{reply}");
    client.say("s2", &session, "二").await;
    home.until_turns(&session, 2).await;
    let requests = script.requests();
    assert!(
        requests[0]
            .1
            .system
            .contains("<mcp-instructions server=\"probe\">"),
        "关之前带着：{}",
        requests[0].1.system
    );
    assert!(
        !requests[1].1.system.contains("<mcp-instructions"),
        "关了以后下一轮拿掉：{}",
        requests[1].1.system
    );
    assert!(
        !menu(&mut client).await.contains(&"probe:hello".to_string()),
        "关了以后菜单里没有它的提示词"
    );
    core.stop_extensions().await;
}

/// 不写会话的命令菜单里的名字。
async fn menu(client: &mut Client) -> Vec<String> {
    let reply = client.call("m", "command.catalog", json!({})).await;
    reply["result"]["commands"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter_map(|one| one["name"].as_str().map(str::to_string))
        .collect()
}

/// 关了的包声明的斜杠命令、整页命令（施工 U23 末下）：菜单里不列，敲了当没有。
#[tokio::test]
async fn a_switched_off_package_s_commands_and_pages_are_gone() {
    let home = Home::new();
    home.write(
        "system/packages/wx/package.toml",
        "[package]\nprotocol = [1, 1]\nname = { en = \"Weather\" }\n\n[command]\nname = \"wx\"\nprogram = \"miyu-wx\"\nabout = { en = \"W\" }\n\n[process]\nstart = \"manual\"\ncapabilities = [\"commands\"]\n\n[[slash]]\nname = \"weather\"\nsummary = { en = \"Today's weather\" }\n",
    );
    home.write(
        "system/packages/wk/package.toml",
        "[package]\nprotocol = [1, 1]\nname = { en = \"Week\" }\n\n[command]\nname = \"wk\"\nprogram = \"miyu-wk\"\nabout = { en = \"W\" }\n\n[process]\nstart = \"always\"\ncapabilities = [\"widgets\"]\n\n[[widget]]\nid = \"week\"\nplace = \"page\"\ntitle = { en = \"This week\" }\ncommand = \"week\"\n",
    );
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let before = menu(&mut client).await;
    assert!(
        before.contains(&"weather".to_string()) && before.contains(&"week".to_string()),
        "关之前都有：{before:?}"
    );
    for (n, id) in [(4, "wx"), (5, "wk")] {
        let reply = set(&mut client, n, &format!("packages.{id}.off"), true).await;
        assert!(reply["result"].is_object(), "{reply}");
    }
    let after = menu(&mut client).await;
    assert!(
        !after.contains(&"weather".to_string()) && !after.contains(&"week".to_string()),
        "关了不列：{after:?}"
    );
    let session = client.create("c1", "~").await;
    for (n, text) in [(1, "/weather 上海"), (2, "/week")] {
        let ran = client
            .call(
                &format!("r{n}"),
                "command.run",
                json!({"session": session, "text": text}),
            )
            .await;
        assert_eq!(
            ran["error"]["data"]["reason"], "unknown_command",
            "敲了当没有：{ran}"
        );
    }
}
