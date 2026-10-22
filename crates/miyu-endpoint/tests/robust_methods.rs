//! 鲁棒性（2026-10-22）：协议的每个方法喂各种不对的参数——缺的、类型不对的、负数、超出范围的大数、超长的字、控制字符、
//! 不存在的编号、深一点的嵌套——每一条都要在时限内回应（结果或拒绝都行），连接不断，核心哪儿都不 panic。
//!
//! 联网的几个方法跳过（测试不连外面）。panic 照全进程的钩子收，所以这个文件单独一个测试程序（`Cargo.toml` 另列 `[[test]]`）。

mod support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};

use support::*;

/// 协议里分发的方法（照 `connection/rights.rs` 的表）。
const METHODS: &[&str] = &[
    "account.logout",
    "account.setup",
    "account.setup_code",
    "blob.close",
    "blob.get",
    "blob.open",
    "blob.put",
    "blob.write",
    "command.call",
    "command.catalog",
    "command.run",
    "config.check",
    "config.get",
    "config.schema",
    "config.set",
    "config.trust",
    "content.list",
    "content.read",
    "events.append",
    "extension.disable",
    "extension.enable",
    "extension.restart",
    "extension.status",
    "feature.set",
    "fs.find",
    "fs.list",
    "fs.read",
    "fs.realpath",
    "hello",
    "human.get",
    "job.output",
    "job.stop",
    "kb.add",
    "kb.create",
    "kb.delete",
    "kb.documents",
    "kb.list",
    "kb.mount",
    "kb.read",
    "kb.reindex",
    "kb.remove",
    "kb.search",
    "kb.update",
    "login.redeem",
    "login.ticket",
    "mcp.list",
    "mcp.remove",
    "mcp.restart",
    "mcp.set",
    "memory.dream",
    "memory.dream_stop",
    "memory.forget",
    "memory.list",
    "memory.remember",
    "memory.restore",
    "memory.search",
    "memory.turns",
    "memory.update",
    "mermaid.render",
    "method.call",
    "model.call",
    "model.embed",
    "model.list",
    "package.call",
    "package.check",
    "package.clean",
    "package.disable",
    "package.enable",
    "package.file",
    "package.files",
    "package.info",
    "package.install",
    "package.list",
    "package.methods",
    "package.owns",
    "package.progress",
    "package.remove",
    "package.restart",
    "persona.avatar",
    "persona.background",
    "persona.delete",
    "persona.get",
    "persona.list",
    "persona.read",
    "persona.set",
    "preset.delete",
    "preset.get",
    "preset.list",
    "preset.set",
    "project.create",
    "project.delete",
    "project.list",
    "project.update",
    "provide",
    "provider.catalog",
    "script.list",
    "search.query",
    "secret.delete",
    "secret.list",
    "secret.set",
    "session.answer",
    "session.clear",
    "session.compact",
    "session.configure",
    "session.create",
    "session.delete",
    "session.fork",
    "session.interrupt",
    "session.list",
    "session.note",
    "session.recap",
    "session.redo",
    "session.respond",
    "session.revert",
    "session.send",
    "session.set_meta",
    "session.set_permission_level",
    "session.set_terminal",
    "session.set_workspace",
    "session.terminal",
    "session.unrevert",
    "skill.list",
    "skill.remove",
    "subscribe",
    "system.append",
    "tool.call",
    "tool.cancel",
    "tool.set_loading",
    "unsubscribe",
    "usage.query",
    "user.append",
    "venue.binding",
    "venue.delivered",
    "venue.recalled",
    "venue.records",
    "venue.session",
    "venue.sessions",
    "view.add",
    "view.append",
    "view.detail",
    "view.hidden",
    "view.page",
    "view.remove",
    "view.update",
    "widget.clear",
    "widget.set",
    "widget.unwatch",
    "widget.watch",
];

/// 嵌套 `depth` 层的对象。
fn nested(depth: usize) -> Value {
    (0..depth).fold(json!("bottom"), |inner, _| json!({ "a": inner }))
}

/// 一个方法要试的几组参数：`session` 是一个真的会话，`work` 是数据根外面的工作目录。
fn variants(session: &str, work: &str) -> Vec<Value> {
    let missing = "0199d1e6-3b7a-7c41-8e5d-2f6b4c9f02a3";
    let huge = u64::MAX;
    let odd = "\u{0}\u{7}\u{1b}[2J\u{202e}\u{200d}🐱‍👤e\u{301}\r\n\t\u{feff}中";
    vec![
        json!({}),
        json!({"session": "not-an-id"}),
        json!({"session": missing, "text": "hi", "stream": "events"}),
        json!({"session": 12345, "text": [], "limit": "ten", "name": 5, "stream": 7}),
        json!({"session": null, "id": null, "name": null, "package": null, "key": null}),
        json!({"session": session, "limit": -1, "after": -1, "page": -1, "offset": -1, "seq": -1}),
        json!({"session": session, "limit": huge, "after": huge, "page": huge, "seq": huge, "turn": huge}),
        json!({"session": session, "limit": 1e300, "page": 1.5, "after": 0.5}),
        json!({"session": session, "stream": "events", "after": huge}),
        json!({
            "name": "../../../x", "package": "../x", "id": "..", "key": "", "path": "../nonexistent-miyu-robust",
            "persona": "..", "preset": "..", "kb": "..", "blob": "sha256:zz", "file": "/..",
            "cwd": "/nonexistent-miyu-robust", "dir": "C:\\..\\x", "url": "file:///etc/passwd",
        }),
        json!({
            "name": "字".repeat(60_000), "text": "x".repeat(300_000), "key": "a.".repeat(5_000),
            "query": odd.repeat(2_000), "package": "p".repeat(10_000),
        }),
        json!({
            "session": session, "text": odd, "name": odd, "key": odd, "query": odd, "title": odd,
            "path": odd, "package": odd, "persona": odd, "kb": odd, "stream": odd,
        }),
        json!({"session": session, "value": nested(100), "data": nested(100), "changes": [{"key": "ui.language", "value": nested(100)}], "layer": "system"}),
        json!({"layer": "system", "text": "[[[[\n=\u{0}", "version": null}),
        json!({"layer": "personal", "changes": [{"key": "", "value": null}, {"key": "ui.language", "input": "\"\u{0}"}]}),
        json!({"cwd": work, "path": work, "dirs": [work, "", "\u{0}"], "session": session}),
    ]
}

/// 发一条请求，等它的回应（中间的推送跳过）：交回回应；`wait` 内没等到的是 `None`。
async fn ask(
    client: &mut Client,
    id: &str,
    method: &str,
    params: &Value,
    wait: Duration,
) -> Option<Value> {
    let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    client.line(&request.to_string()).await;
    let deadline = tokio::time::Instant::now() + wait;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let next = client.next_within(left).await?;
        if next["id"] == json!(id) {
            return Some(next);
        }
    }
}

/// 照写法截一段给人看的参数。
fn shown(params: &Value) -> String {
    params.to_string().chars().take(120).collect()
}

#[tokio::test]
async fn every_method_answers_odd_params_and_keeps_the_connection() {
    let panics = Arc::new(Mutex::new(Vec::<String>::new()));
    let seen = Arc::clone(&panics);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        seen.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(info.to_string());
    }));

    let home = Home::new();
    let core = home.core(&Script::new((0..500).map(|_| Play::Says("好"))));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let mut session = client.create("c0", &work).await;
    let mut problems = Vec::new();
    let mut n = 0;
    for method in METHODS {
        for params in variants(&session, &work) {
            n += 1;
            let id = format!("r{n}");
            if ask(&mut client, &id, method, &params, Duration::from_secs(20))
                .await
                .is_some()
            {
                continue;
            }
            let alive = ask(
                &mut client,
                &format!("p{n}"),
                "human.get",
                &json!({}),
                Duration::from_secs(5),
            )
            .await
            .is_some();
            problems.push(format!(
                "{method} {}：{}",
                shown(&params),
                if alive {
                    "20 秒没回"
                } else {
                    "连接断了"
                }
            ));
            if !alive {
                client = Client::connect(Arc::clone(&core));
                client.hello().await;
            }
        }
        // 删会话这类方法把那个会话用掉了：换一个新的接着试。
        if *method == "session.delete" || *method == "session.clear" {
            n += 1;
            session = client.create(&format!("c{n}"), &work).await;
        }
    }
    // 喂完以后核心照常：新连上的能握手、能造会话。
    let mut fresh = Client::connect(Arc::clone(&core));
    fresh.hello().await;
    let made = fresh
        .call("last", "session.create", json!({"cwd": work}))
        .await;
    if made.get("result").is_none() {
        problems.push(format!("喂完以后造不了会话：{made}"));
    }
    std::panic::set_hook(previous);
    let panics = panics
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert!(
        problems.is_empty() && panics.is_empty(),
        "试了 {n} 条。\n没回、断了的 {} 条：\n{}\n\npanic {} 次：\n{}",
        problems.len(),
        problems.join("\n"),
        panics.len(),
        panics.join("\n---\n")
    );
}
