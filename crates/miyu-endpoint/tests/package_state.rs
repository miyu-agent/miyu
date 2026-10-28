//! 包的会话状态（施工 TW-1，设计 `46-包的会话状态.md`）：真核心拉起测试用的扩展。程序直接写（`session.set_state`）的记一条
//! `package.state`、读得回（`session.state`）；写了没声明成状态块的块的拒 `bad_params`；工具结果带的记进效果，没声明的块的整份丢掉。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, Effect};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

fn tools() -> Value {
    let spec = |name: &str| {
        json!({"name": name, "description": "A test tool.", "input_schema": {"type": "object"},
               "access": "read", "venues": ["local"]})
    };
    json!({"tools": [spec("state"), spec("bad_state")]})
}

/// 等到 `path` 里记下的有 `n` 行。
async fn lines(path: &std::path::Path, n: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(path).lines().count() < n {
        assert!(tokio::time::Instant::now() < deadline, "等不到 {n} 行");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn a_package_writes_reads_and_returns_its_session_state() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "todo");
    let go = home.root.path().join("go");
    let written = r#"ask:session.set_state:{"session":"{session}","state":{"widgets":{"todos":{"content":[{"type":"text","text":"from the program"}]}},"data":{"n":1}}}"#;
    let undeclared = r#"ask:session.set_state:{"session":"{session}","state":{"widgets":{"plain":{"content":[]}}}}"#;
    install_asking(
        &home,
        "todo",
        &program.name(),
        "always",
        &steps(&[
            &step,
            "hello",
            &format!("ask:provide:{}", tools()),
            &format!("session:{}", go.display()),
            written,
            r#"ask:session.state:{"session":"{session}"}"#,
            undeclared,
            "serve",
        ]),
        &["tools", "widgets", "sessions.drive"],
    );
    let manifest = home.root.path().join("packages/todo/package.toml");
    let mut text = std::fs::read_to_string(&manifest).expect("读得了");
    text.push_str("\n[[widget]]\nid = \"todos\"\nplace = \"sidebar\"\ntitle = { en = \"Todos\" }\nscope = \"session\"\nstate = true\n\n[[widget]]\nid = \"plain\"\nplace = \"sidebar\"\ntitle = { en = \"Plain\" }\nscope = \"session\"\n");
    std::fs::write(&manifest, text).expect("写得进");
    approve(&home, "todo", &["tools", "widgets", "sessions.drive"]);
    let script = Script::new([
        Play::calls(&[("state", "{}")]),
        Play::calls(&[("bad_state", "{}")]),
        Play::Says("好。"),
    ])
    .titles([Play::Says("测试")]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    lines(&path, 3).await;
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    std::fs::write(&go, &session).expect("写得进");
    lines(&path, 6).await;

    let recorded: Vec<Value> = read(&path)
        .lines()
        .skip(3)
        .map(|line| serde_json::from_str(line).expect("一行 JSON"))
        .collect();
    assert_eq!(recorded[0]["result"], json!({}), "{}", recorded[0]);
    assert_eq!(
        recorded[1]["result"]["state"]["data"],
        json!({"n": 1}),
        "{}",
        recorded[1]
    );
    assert_eq!(
        recorded[1]["result"]["state"]["widgets"]["todos"]["content"][0]["text"],
        "from the program"
    );
    assert_eq!(
        recorded[2]["error"]["data"]["reason"], "bad_params",
        "{}",
        recorded[2]
    );
    assert_eq!(
        recorded[2]["error"]["data"]["field"], "state.widgets.plain",
        "没写 state = true 的块不收"
    );
    let written: Vec<String> = home
        .log(&session)
        .iter()
        .filter_map(|event| match &event.body {
            Body::PackageState(state) => Some(state.package.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(written, ["todo"], "程序写的记一条，拒的不记");

    client.say("s1", &session, "记一下").await;
    home.until_turns(&session, 1).await;
    let effects: Vec<Vec<Effect>> = home
        .log(&session)
        .iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result.effects.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(effects.len(), 2, "{effects:?}");
    let Some(Effect::PackageState(noted)) = effects[0].first() else {
        panic!("工具结果带的记进效果：{effects:?}");
    };
    assert_eq!(noted.package, "todo");
    assert_eq!(noted.state.note.as_deref(), Some("Noted by the tool."));
    assert!(
        effects[1].is_empty(),
        "写了没声明的块的整份丢掉：{effects:?}"
    );
}
