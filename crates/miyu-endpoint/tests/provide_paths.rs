//! 扩展的工具在规格里的两样声明（施工 XG-1 上，设计 `63-扩展工具的效果和声明.md` 第二、三、六节）：真核心走一遍。
//!
//! - `paths`：和内置 `edit` 走同一张判法表（以前扩展的工具不报路径，照「不报路径的调用」放行）：写进数据根的当场拒，扩展收不到
//!   `tool.call`；放行的 `tool.call` 带 `checked`，是链判过的真实位置（A16 起完全放开下写工作区外不再问人，施工 LV-1）；
//! - `background_parameters`：后台运行关着的会话，工具面上去掉那几个参数；开着的照旧。不写这几格的工具，参数格式逐字节照登记的。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::request::Request;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::venues::configured_core;
use crate::support::*;

/// 等到 `path` 里记下的有 `n` 行。
async fn lines(path: &Path, n: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(path).lines().count() < n {
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {n} 行：{}",
            read(path)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 扩展收到的 `tool.call`，照先后。
fn calls(path: &Path) -> Vec<Value> {
    read(path)
        .lines()
        .filter(|line| line.contains(r#""method":"tool.call""#))
        .map(|line| serde_json::from_str(line).expect("一行 JSON"))
        .collect()
}

/// 日志里的确认请求有几条。
fn asked(home: &Home, session: &str) -> usize {
    logged(home, session)
        .iter()
        .filter(|event| event["kind"] == json!("tool.approval_requested"))
        .count()
}

#[tokio::test]
async fn a_declared_write_into_the_data_root_is_refused_before_the_extension_hears_of_it() {
    let home = Home::new();
    let program = Program::new();
    let (path, keep) = record(&home, "writer");
    let offered = json!({"tools": [{
        "name": "ext_write", "description": "Write a file.",
        "input_schema": {"type": "object", "properties": {"file_path": {"type": "string"}},
                         "required": ["file_path"]},
        "access": "write", "venues": ["local"],
        "paths": [{"parameter": "file_path", "write": true}],
    }]});
    install_providing(
        &home,
        "writer",
        &program.name(),
        "always",
        &steps(&[&keep, "hello", &format!("ask:provide:{offered}"), "serve"]),
    );
    let outside = home
        .root
        .path()
        .join("x.txt")
        .to_string_lossy()
        .into_owned();
    let script = Script::new([
        Play::calls(&[("ext_write", r#"{"file_path":"inside.txt"}"#)]),
        Play::calls(&[("ext_write", &json!({ "file_path": outside }).to_string())]),
        Play::Says("写了。"),
    ]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    lines(&path, 3).await;
    assert!(
        read(&path).contains(r#""tools":1"#),
        "登记上了：{}",
        read(&path)
    );

    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "写一下").await;
    home.until_turns(&session, 1).await;
    assert_eq!(asked(&home, &session), 0, "数据根不问人，直接拒");
    let sent = calls(&path);
    assert_eq!(
        sent.len(),
        1,
        "工作区里的照常发；数据根里的扩展收不到：{sent:?}"
    );
    let inside = std::fs::canonicalize(&home.work)
        .expect("在")
        .join("inside.txt");
    assert_eq!(
        sent[0]["params"]["checked"],
        json!([miyu_fs::plain(&inside)]),
        "带着链判过的真实位置"
    );
    assert!(!Path::new(&outside).exists());
}

/// 第一次请求里 `name` 那件工具的参数格式，原文。
fn parameters(request: &Request, name: &str) -> Option<String> {
    request
        .tools
        .iter()
        .find(|tool| tool.name == name)
        .map(|tool| tool.parameters.get().to_string())
}

/// 起核心、拉起登记 `offered` 的扩展、造一个用预设 `preset` 的会话、说一句，交回第一次请求。
async fn first_request(preset: &str, offered: &Value) -> Request {
    let home = Home::new();
    home.write(
        "root/presets/quiet.toml",
        "[preset]\nname = { en = \"Quiet\" }\n\n[features]\njobs = false\n",
    );
    let program = Program::new();
    let (path, keep) = record(&home, "runner");
    install_providing(
        &home,
        "runner",
        &program.name(),
        "always",
        &steps(&[&keep, "hello", &format!("ask:provide:{offered}"), "serve"]),
    );
    let tools =
        Catalog::in_packages(miyu_basesystem::packages(&default_resources()).expect("出厂的工具"))
            .expect("合写法");
    let script = Script::new([Play::Says("嗯。")]);
    let core = configured_core(&home, &script, tools);
    core.start_extensions();
    lines(&path, 3).await;
    let mut client = Client::connect(core);
    client.hello().await;
    let reply = client
        .call(
            "c1",
            "session.create",
            json!({"cwd": "~", "persona": "engineer", "preset": preset}),
        )
        .await;
    let session = reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("没造出来：{reply}"))
        .to_string();
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    script.requests()[0].1.clone()
}

#[tokio::test]
async fn background_parameters_are_left_out_without_a_background() {
    let run_schema = json!({"type": "object", "properties": {
        "command": {"type": "string"}, "in_background": {"type": "boolean"},
    }, "required": ["command"]});
    let plain_schema = json!({"type": "object", "properties": {
        "query": {"type": "string"}, "limit": {"type": "integer"},
    }, "required": ["query"]});
    let offered = json!({"tools": [
        {"name": "ext_run", "description": "Run a thing.", "input_schema": run_schema,
         "access": "network", "venues": ["local"], "background_parameters": ["in_background"]},
        {"name": "ext_find", "description": "Find a thing.", "input_schema": plain_schema,
         "access": "read", "venues": ["local"]},
    ]});
    let quiet = first_request("quiet", &offered).await;
    let run: Value =
        serde_json::from_str(&parameters(&quiet, "ext_run").expect("有 ext_run")).expect("JSON");
    assert!(run["properties"].get("in_background").is_none(), "{run}");
    assert_eq!(run["properties"]["command"], json!({"type": "string"}));
    // 不写新格的：参数格式逐字节照登记的（同以前）。
    assert_eq!(
        parameters(&quiet, "ext_find").expect("有 ext_find"),
        plain_schema.to_string()
    );

    let full = first_request("full", &offered).await;
    assert_eq!(
        parameters(&full, "ext_run").expect("有 ext_run"),
        run_schema.to_string(),
        "开着后台的照旧"
    );
}
