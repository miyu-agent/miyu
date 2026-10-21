//! MCP 服务的提示词当斜杠命令（施工 X-39 之二，`docs/construction/X-39-MCP的说明和提示词.md`）：真核心真起 `miyu-test-mcp`。
//! `command.catalog` 列出 `<服务名>:<提示词名>`、参数照必填可选拼，名字清理以后撞了的不登记、软件包列表的 `skipped` 里有它；
//! `command.run` 照先后对上参数、最后一个拿剩下的，取回来的字、图并成这个人说的一句（`assistant` 的照原文并进来），时间线是人敲
//! 的那一行；缺必填的、服务回错的 `command_failed` 带原话，没列出的 `command_unavailable`，没有的服务 `unknown_command`；预设
//! 关了这个服务的拒；服务停了不列。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, MessageUser, TypedCommand};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::mcp::*;
use crate::support::*;

/// 核心：照 `script` 答，等法短；包 `probe` 带一个声明了提示词的服务，开着、在跑。
async fn running(home: &Home, script: &Script) -> (Arc<Core>, Client) {
    bundle(home, "probe", "probe", &["prompts"]);
    let core = Arc::new(
        home.core_full(script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    enable(&mut client, "probe").await;
    until_status(&mut client, "probe", "running").await;
    (core, client)
}

async fn run(client: &mut Client, id: &str, session: &str, text: &str) -> Value {
    client
        .call(id, "command.run", json!({"session": session, "text": text}))
        .await
}

/// 菜单里名字是 `name` 的那一项。
async fn item(client: &mut Client, session: Option<&str>, name: &str) -> Option<Value> {
    let params = session.map_or_else(|| json!({}), |session| json!({"session": session}));
    let listed = client.call("c", "command.catalog", params).await;
    listed["result"]["commands"]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"))
        .iter()
        .find(|one| one["name"] == name)
        .cloned()
}

#[tokio::test]
async fn prompts_are_listed_as_slash_commands() {
    let home = Home::new();
    let (core, mut client) = running(&home, &Script::new([])).await;
    let session = open(&mut client).await;
    assert_eq!(
        item(&mut client, Some(&session), "probe:review").await,
        Some(
            json!({"name": "probe:review", "aliases": [], "summary": "Review code", "argument": "<code> [focus]"})
        )
    );
    assert_eq!(
        item(&mut client, None, "probe:hello").await,
        Some(json!({"name": "probe:hello", "aliases": [], "summary": "Say hello."})),
        "不写会话的照管理员列"
    );
    assert!(
        item(&mut client, None, "probe:two_words").await.is_some(),
        "名字里的空白清理成 _"
    );
    let entry = entry(&mut client, "probe").await;
    assert_eq!(
        entry["mcp"][0]["skipped"],
        json!([{"prompt": "two_words", "why": "name_taken"}]),
        "清理以后撞名的不登记：{entry}"
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn a_prompt_is_said_as_one_line_with_its_arguments() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let (core, mut client) = running(&home, &script).await;
    let session = open(&mut client).await;
    let reply = run(
        &mut client,
        "k1",
        &session,
        "/probe:review main.rs naming and tests",
    )
    .await;
    assert_eq!(reply["result"]["command"], "probe:review", "{reply}");
    assert_eq!(reply["result"]["said"], "", "{reply}");
    home.until_turns(&session, 1).await;
    let said: Vec<(Vec<Block>, Option<TypedCommand>)> = home
        .log(&session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::MessageUser(MessageUser {
                blocks, command, ..
            }) => Some((blocks, command)),
            _ => None,
        })
        .collect();
    assert_eq!(said.len(), 1, "{said:?}");
    let (blocks, command) = &said[0];
    assert_eq!(
        command,
        &Some(TypedCommand {
            name: "probe:review".to_string(),
            text: "/probe:review main.rs naming and tests".to_string(),
        })
    );
    let texts: Vec<&str> = blocks
        .iter()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        texts,
        ["Review this code for naming and tests:\nmain.rs", "Sure."],
        "第一个参数照空白切出一个词，最后一个拿剩下的；她说的照原文并进来"
    );
    assert!(
        blocks.iter().any(|block| matches!(block, Block::Image(_))),
        "图存成 blob、接在后面：{blocks:?}"
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn refusals_say_why() {
    let home = Home::new();
    let (core, mut client) = running(&home, &Script::new([])).await;
    let session = open(&mut client).await;
    let missing = run(&mut client, "k1", &session, "/probe:review").await;
    assert_eq!(reason(&missing), Some("command_failed"), "{missing}");
    let why = missing["error"]["data"]["why"].as_str().unwrap_or_default();
    assert!(why.contains("code"), "说缺哪个：{missing}");
    let unlisted = run(&mut client, "k2", &session, "/probe:nope").await;
    assert_eq!(reason(&unlisted), Some("command_unavailable"), "{unlisted}");
    let unknown = run(&mut client, "k3", &session, "/nobody:review").await;
    assert_eq!(reason(&unknown), Some("unknown_command"), "{unknown}");
    assert!(
        !home
            .log(&session)
            .iter()
            .any(|event| matches!(event.body, Body::MessageUser(_))),
        "拒了的什么都不说"
    );

    // 服务停了：菜单不列，跑的 `command_unavailable`。
    let disabled = client
        .call("d", "package.disable", json!({"package": "probe"}))
        .await;
    assert!(disabled["result"].is_object(), "{disabled}");
    until_status(&mut client, "probe", "off").await;
    assert_eq!(
        item(&mut client, Some(&session), "probe:review").await,
        None
    );
    let stopped = run(&mut client, "k4", &session, "/probe:hello").await;
    assert_eq!(reason(&stopped), Some("command_unavailable"), "{stopped}");
    core.stop_extensions().await;
}

#[tokio::test]
async fn a_preset_turning_the_service_off_turns_its_prompts_off() {
    let home = Home::new();
    home.write(
        "home/alice/presets/quiet.toml",
        "[preset]\nname = \"安静\"\n\n[features]\nmcp-probe = false\n",
    );
    let (core, mut client) = running(&home, &Script::new([])).await;
    let session = client
        .create_with("c1", json!({"cwd": "~", "preset": "quiet"}))
        .await;
    let reply = run(&mut client, "k1", &session, "/probe:hello").await;
    assert_eq!(reason(&reply), Some("command_not_allowed"), "{reply}");
    assert_eq!(item(&mut client, Some(&session), "probe:hello").await, None);
    core.stop_extensions().await;
}
