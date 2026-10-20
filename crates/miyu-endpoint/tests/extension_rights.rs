//! 扩展照能力挡（施工 X-30，`docs/blueprint/extensions.md`「谁能调」）：核心拉起的扩展调只给人的回 `local_only`，缺能力的回
//! `needs_approval`、`data.capabilities` 是缺的那一项，表里没有的当只给人；声明了的照常；扩展不带 `as` 说的话记成名字是包编号的
//! `harness`，自己写 `from` 参数不对。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_kernel::origin::By;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::{Program, install_asking, quick, read, record, steps};
use crate::support::*;

/// 等记的文件里有 `n` 行（头一行是工作目录）。
async fn lines(path: &Path, n: usize) -> Vec<Value> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let text = read(path);
        if text.lines().count() >= n {
            return text
                .lines()
                .skip(1)
                .map(|line| serde_json::from_str(line).expect("回应是 JSON"))
                .collect();
        }
        assert!(tokio::time::Instant::now() < deadline, "{text}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 拒的原因码。
fn reason(reply: &Value) -> &str {
    reply["error"]["data"]["reason"]
        .as_str()
        .unwrap_or_default()
}

/// 装上 `id`（声明 `capabilities`、算批过），照 `asks` 一步步调，最后等着；交回记的文件。
fn install(
    home: &Home,
    program: &Program,
    id: &str,
    capabilities: &[&str],
    asks: &[&str],
) -> std::path::PathBuf {
    let (path, keep) = record(home, id);
    let mut all = vec![keep.as_str(), "hello"];
    all.extend_from_slice(asks);
    all.push("wait");
    install_asking(
        home,
        id,
        &program.name(),
        "always",
        &steps(&all),
        capabilities,
    );
    path
}

/// 起核心：`approved` 是各包批过的能力。
fn core(home: &Home, script: &Script, approved: Value) -> Arc<miyu_endpoint::Core> {
    let system = home.root.system();
    std::fs::create_dir_all(&system).expect("建得了");
    std::fs::write(
        system.join("extensions.json"),
        format!(
            "{}\n",
            json!({"version": 1, "on": {}, "approved": approved})
        ),
    )
    .expect("写得进");
    let core = Arc::new(
        home.core_full(script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    core
}

#[tokio::test]
async fn extensions_only_reach_what_they_declared() {
    let home = Home::new();
    let program = Program::new();
    let asks = [
        // 只给人的：替人回答确认、改权限级别、删会话、表里没有的。
        r#"ask:session.answer:{"session":"x","call":"c","decision":"once"}"#,
        r#"ask:session.set_permission_level:{"session":"x","level":"full"}"#,
        r#"ask:session.delete:{"session":"x"}"#,
        r#"ask:session.set_workspace:{"session":"x","cwd":"~"}"#,
        "call:no.such_method",
        // 要能力的：参数没查之前就挡下。
        r#"ask:session.create:{"cwd":"~"}"#,
        r#"ask:session.send:{"session":"x","text":"hi"}"#,
        r#"ask:session.send:{"session":"x","text":"hi","as":{"platform":"qq","id":"1","name":"a"}}"#,
        r#"ask:session.note:{"session":"x","facts":[{"kind":"k","text":"t"}]}"#,
        r#"ask:subscribe:{"stream":"events","session":"x"}"#,
        r#"ask:events.append:{"session":"x","kind":"ext.plain.a","body":{}}"#,
        r#"ask:memory.remember:{"text":"t"}"#,
        r#"ask:fs.read:{"path":"/"}"#,
        // 都能调的：照现在，这里只看没被挡。
        "call:model.list",
    ];
    let path = install(&home, &program, "plain", &["tools"], &asks);
    let _core = core(&home, &Script::new([]), json!({"plain": ["tools"]}));
    let replies = lines(&path, 2 + asks.len()).await;
    let replies = &replies[1..];
    for (n, ask) in asks.iter().enumerate().take(5) {
        assert_eq!(reason(&replies[n]), "local_only", "{ask}：{}", replies[n]);
    }
    let needs = [
        "sessions.drive",
        "sessions.drive",
        "act_for_external",
        "act_for_external",
        "events.read",
        "events.write",
        "memory.write",
        "fs.read",
    ];
    for (n, capability) in needs.iter().enumerate() {
        let reply = &replies[5 + n];
        assert_eq!(reason(reply), "needs_approval", "{}：{reply}", asks[5 + n]);
        assert_eq!(
            reply["error"]["data"]["capabilities"],
            json!([capability]),
            "{}：{reply}",
            asks[5 + n]
        );
    }
    let open = &replies[5 + needs.len()];
    assert!(open.get("result").is_some(), "读模型目录不挡：{open}");
}

#[tokio::test]
async fn an_extension_speaks_as_itself() {
    let home = Home::new();
    let program = Program::new();
    let asks = [
        r#"ask:session.create:{"cwd":"~"}"#,
        r#"ask:session.send:{"session":"{session}","text":"扩展说的。"}"#,
        r#"ask:session.send:{"session":"{session}","text":"冒充。","from":"claude-code"}"#,
        r#"ask:subscribe:{"stream":"events","session":"{session}"}"#,
    ];
    let declared = ["sessions.drive", "events.read"];
    let path = install(&home, &program, "driver", &declared, &asks);
    let _core = core(
        &home,
        &Script::new([Play::Says("好。")]),
        json!({"driver": declared}),
    );
    let replies = lines(&path, 2 + asks.len()).await;
    let created = &replies[1];
    let session = created["result"]["session"]
        .as_str()
        .expect("造得了会话")
        .to_string();
    assert!(
        replies[2].get("result").is_some(),
        "声明了就照常：{}",
        replies[2]
    );
    assert_eq!(
        reason(&replies[3]),
        "bad_params",
        "扩展自己写 from 冒充不了：{}",
        replies[3]
    );
    assert!(
        replies[4].get("result").is_some(),
        "声明了 events.read 能订阅：{}",
        replies[4]
    );
    home.until_turns(&session, 1).await;
    let by: Vec<By> = home
        .log(&session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::MessageUser(_) => Some(event.by),
            _ => None,
        })
        .collect();
    let driver: By =
        serde_json::from_value(json!({"kind": "harness", "name": "driver"})).expect("合写法");
    assert_eq!(by, [driver], "记成它自己，不是管理员");
}
