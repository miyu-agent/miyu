//! 工具声明写错了的复核（施工 XG-2，`docs/designs/64-工具职能.md` 第三节；测试会话 e0 的对抗式复核）：扩展经 `provide` 交认不得的
//! `role`、`scope`（连大小写写错的）整个不收、回 `bad_tool` 说是哪一格；脚本开头写错的不登记，`check` 报 `script_skipped`、说是哪
//! 一行。写工具声明 `read_file` 照收：职能只管技能列不列，不另给待遇（设计 64 第四节）。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::scripts::core as script_core;
use crate::support::extensions::*;
use crate::support::*;

/// 一件工具的规格，另加 `extra` 里的几格。
fn spec(name: &str, access: &str, extra: Value) -> Value {
    let mut spec = json!({"name": name, "description": format!("The {name} tool."),
        "input_schema": {"type": "object"}, "access": access, "venues": ["local"]});
    for (key, value) in extra.as_object().expect("是对象") {
        spec[key] = value.clone();
    }
    spec
}

#[tokio::test]
async fn a_wrong_role_or_scope_from_an_extension_is_refused_by_field() {
    let home = Home::new();
    let program = Program::new();
    let (path, keep) = record(&home, "decl");
    let cases = [
        spec("w_role", "read", json!({"role": "writer"})),
        spec("w_scope", "read", json!({"scope": "everywhere"})),
        spec("w_case", "read", json!({"scope": "Local"})),
        spec("w_type", "read", json!({"scope": ["local"]})),
        spec(
            "writes",
            "write",
            json!({"role": "read_file", "scope": "local"}),
        ),
    ];
    let mut args = vec![keep.clone(), "hello".to_string()];
    args.extend(
        cases
            .iter()
            .map(|tool| format!("ask:provide:{}", json!({"tools": [tool]}))),
    );
    args.push("wait".to_string());
    install_providing(&home, "decl", &program.name(), "always", &args);
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    // 记下的：工作目录、握手、五次登记的回应。
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
    let got = loop {
        let got: Vec<String> = read(&path).lines().map(str::to_string).collect();
        if got.len() >= 7 {
            break got;
        }
        assert!(tokio::time::Instant::now() < deadline, "等不到：{got:?}");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    };
    let replies: Vec<Value> = got[2..7]
        .iter()
        .map(|line| serde_json::from_str(line).expect("一行 JSON"))
        .collect();
    for (reply, problem) in replies[..4].iter().zip(["role", "scope", "scope", "scope"]) {
        assert_eq!(reply["error"]["data"]["reason"], "bad_tool", "{reply}");
        assert_eq!(reply["error"]["data"]["problem"], problem, "{reply}");
    }
    assert_eq!(
        replies[4]["result"],
        json!({"tools": 1}),
        "写工具声明 read_file 照收：{}",
        replies[4]
    );
}

#[tokio::test]
async fn a_script_with_a_wrong_role_or_scope_is_skipped_and_checked() {
    let home = Home::new();
    let head = |name: &str, line: &str| {
        format!("#!/usr/bin/env python3\n# Description: {name}.\n# {line}\nprint('{name}')\n")
    };
    home.write("root/scripts/far.py", &head("far", "Scope: everywhere"));
    home.write("root/scripts/odd.py", &head("odd", "Role: writer"));
    home.write("root/scripts/fine.py", &head("fine", "Scope: local"));
    let core = script_core(&home, &Script::new([]));
    let mut names: Vec<String> = core
        .scripts()
        .iter()
        .map(|one| one.head.name.clone())
        .collect();
    names.sort();
    assert_eq!(names, ["fine"], "写错的不登记，写对的登记上");
    let mut client = Client::connect(core);
    client.hello().await;
    let reply = client.call("k1", "check", json!({})).await;
    let mut skipped: Vec<(String, String)> = reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|one| one["code"] == "script_skipped")
        .map(|one| {
            let text = |key: &str| one[key].as_str().unwrap_or_default().to_string();
            (text("file"), text("message"))
        })
        .collect();
    skipped.sort();
    assert_eq!(skipped.len(), 2, "写对的那份不报：{reply}");
    assert!(skipped[0].0.ends_with("far.py"), "{skipped:?}");
    assert!(
        skipped[0].1.contains("everywhere"),
        "说是哪一行：{skipped:?}"
    );
    assert!(skipped[1].0.ends_with("odd.py"), "{skipped:?}");
    assert!(skipped[1].1.contains("writer"), "说是哪一行：{skipped:?}");
}
