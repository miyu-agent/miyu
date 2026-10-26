//! fork 子代理，真核心走一遍（施工 A-F 下，`docs/blueprint/agents.md`「一补、fork 子代理」）：主会话一步里派两个 fork 子代理，
//! 会话表照主会话落了盘的日志分叉：两个子会话的快照就是主会话那一份，头一次请求的 system、工具面一样，消息除了各自的交代
//! 一字不差，开头和主会话的一样；各自报回来；重启以后照样载入。
//!
//! 几个会话同时请求模型：替身照请求里人这边的话分剧本。fork 出来的请求里也有主会话那一句，所以两份交代排在前面先认。

use std::sync::Arc;

use serde_json::json;

use crate::support::{Client, Home, default_resources};
use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Effect, Event, Purpose};
use miyu_kernel::id::{Seq, SessionId};
use miyu_kernel::request::{Message, Request};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Cancel, ForSession, ModelPort, Models, Reports};
use miyu_tool::Catalog;

/// 各个会话的剧本：请求里人这边的话含哪一句，就照哪一份回，照先后认。
#[derive(Clone)]
struct Router(Arc<Vec<(&'static str, Script)>>);

impl Router {
    fn script(&self, request: &Request) -> &Script {
        let said = |key: &str| {
            request.messages.iter().any(|message| match message {
                Message::User { blocks, .. } => blocks.iter().any(|block| match block {
                    Block::Text(text) => text.text.contains(key),
                    _ => false,
                }),
                _ => false,
            })
        };
        self.0
            .iter()
            .find(|(key, _)| said(key))
            .map(|(_, script)| script)
            .unwrap_or_else(|| panic!("没有哪份剧本认这次请求"))
    }
}

impl Models for Router {
    fn port(&self, _: ForSession) -> Arc<dyn ModelPort> {
        Arc::new(self.clone())
    }
}

impl ModelPort for Router {
    fn model(&self) -> miyu_kernel::origin::Model {
        self.0[0].1.model()
    }

    fn call(
        &self,
        seen: Seq,
        request: Request,
        config: &miyu_session::TurnConfig,
        reports: Reports,
        cancel: Cancel,
    ) {
        // 起标题的请求不带哪一份的原话：不回，一直在路上，不碍这里测的。
        if reports.purpose() == Some(&Purpose::Title) {
            return;
        }
        let script = self.script(&request).clone();
        script.call(seen, request, config, reports, cancel);
    }
}

/// fork 一个子代理，交代是 `prompt`。
fn fork(prompt: &str) -> String {
    json!({"description": "查", "prompt": prompt, "fork": true}).to_string()
}

/// 日志里 `job.started` 记着的子会话，照先后。
fn started_children(log: &[Event]) -> Vec<SessionId> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(&result.effects),
            _ => None,
        })
        .flatten()
        .filter_map(|effect| match effect {
            Effect::JobStarted(started) => started.session.clone(),
            _ => None,
        })
        .collect()
}

/// 日志里有几条回报。
fn reports(log: &[Event]) -> usize {
    log.iter()
        .filter(|event| matches!(event.body, Body::ChildReported(_)))
        .count()
}

/// 第一条（造会话）。
fn created(log: &[Event]) -> miyu_kernel::event::SessionCreated {
    match &log[0].body {
        Body::SessionCreated(created) => created.clone(),
        other => panic!("第 1 条应该是造会话：{other:?}"),
    }
}

#[tokio::test]
async fn two_forks_from_one_step_share_the_parents_prefix_and_report() {
    let home = Home::new();
    let tools = || Catalog::new(miyu_basesystem::tools(&default_resources()).unwrap()).unwrap();
    let main = Script::new([
        Play::calls(&[
            ("subagent", &fork("TASK-A: count the files.")),
            ("subagent", &fork("TASK-B: name the biggest file.")),
        ]),
        Play::Says("派出去了。"),
        Play::Says("收到一个。"),
        Play::Says("都收到了。"),
    ]);
    let (task_a, task_b) = (
        Script::new([Play::Says("A 做完了。")]),
        Script::new([Play::Says("B 做完了。")]),
    );
    let router = Router(Arc::new(vec![
        ("TASK-A", task_a.clone()),
        ("TASK-B", task_b.clone()),
        ("派两个去查", main.clone()),
    ]));
    let core = home.core_with_models(Arc::new(router), tools());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let parent = client.create("c1", &work).await;
    client.say("c2", &parent, "派两个去查").await;
    home.until_turns(&parent, 1).await;
    // 两份都报回来了。
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
    while reports(&home.log(&parent)) < 2 {
        assert!(tokio::time::Instant::now() < deadline, "等不到两份回报");
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    let log = home.log(&parent);
    let children = started_children(&log);
    assert_eq!(children.len(), 2);
    let step = log
        .iter()
        .find(|event| matches!(event.body, Body::MessageAssistant(_)))
        .map(|event| event.seq)
        .expect("派它们的那条回复");
    let snapshot = created(&log).policy;
    for child in &children {
        let child_log = home.log(child.as_str());
        let made = created(&child_log);
        let forked = made.forked.expect("分叉出来的");
        assert_eq!(forked.upto, step, "从派它的那一步分叉");
        assert!(forked.within.is_some());
        assert_eq!(made.policy, snapshot, "快照就是主会话那一份");
        assert_eq!(made.depth, Some(1));
        assert!(
            !child_log
                .iter()
                .any(|event| matches!(event.body, Body::PolicyChanged(_))),
            "回合开始不换快照"
        );
    }
    let (a, b) = (&task_a.requests()[0].1, &task_b.requests()[0].1);
    let parent_next = &main.requests()[1].1;
    assert_eq!(a.system, b.system);
    assert_eq!(a.tools, b.tools);
    assert_eq!(a.system, parent_next.system, "system 和主会话的一样");
    assert_eq!(a.tools, parent_next.tools, "工具面和主会话的一样");
    let n = a.messages.len();
    assert_eq!(b.messages.len(), n);
    assert_eq!(
        a.messages[..n - 1],
        b.messages[..n - 1],
        "两个兄弟只差各自的交代"
    );
    assert_ne!(a.messages[n - 1], b.messages[n - 1]);
    assert_eq!(
        a.messages[..2],
        parent_next.messages[..2],
        "人那一句、派它们的那条回复和主会话的一样"
    );

    // 头看 fork 子会话：视图流订得上，按页读的条目里有交代开的那一轮和它的回答（头几条的占位结果、收住那一轮投影得了）。
    let view = client
        .call(
            "v1",
            "subscribe",
            json!({"session": children[0], "stream": "view"}),
        )
        .await;
    assert!(view.get("error").is_none(), "{view}");
    let page = client
        .call(
            "p1",
            "view.page",
            json!({"session": children[0], "view": true}),
        )
        .await;
    assert!(page.get("error").is_none(), "{page}");
    let entries = page["result"]["entries"].to_string();
    // 两个谁先派完不一定（结果照到的先后记），只认它的回答在。
    assert!(entries.contains("做完了。"), "{page}");
    assert!(
        entries.contains("You are a fork of the agent above."),
        "{page}"
    );

    // 头认得出 fork 的（施工 A-F 补）：会话表那一行写 `forked: true`，主会话那一行不写；主会话的任务表里两项都是 `fork: true`。
    let listed = client.call("l1", "session.list", json!({})).await;
    let sessions = listed["result"]["sessions"]
        .as_array()
        .unwrap_or_else(|| panic!("有会话：{listed}"))
        .clone();
    for child in &children {
        let row = sessions
            .iter()
            .find(|row| row["session"] == json!(child))
            .unwrap_or_else(|| panic!("列着 fork 子会话：{listed}"));
        assert_eq!(row["forked"], json!(true), "{row}");
    }
    let main_row = sessions
        .iter()
        .find(|row| row["session"] == json!(parent))
        .unwrap_or_else(|| panic!("列着主会话：{listed}"));
    assert!(main_row.get("forked").is_none(), "{main_row}");
    let forks: Vec<bool> = log
        .iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(&result.effects),
            _ => None,
        })
        .flatten()
        .filter_map(|effect| match effect {
            Effect::JobStarted(started) => Some(started.fork),
            _ => None,
        })
        .collect();
    assert_eq!(forks, [true, true], "job.started 写 fork");

    // 重启以后照样载入：分叉点在一轮当中的底子照样认得。
    core.stop_sessions().await;
    drop(client);
    let after = Router(Arc::new(vec![(
        "TASK-A",
        Script::new([Play::Says("又做了一遍。")]),
    )]));
    let mut client = Client::connect(home.core_with_models(Arc::new(after), tools()));
    client.hello().await;
    let subscribed = client.subscribe("w1", children[0].as_str()).await;
    assert!(subscribed.get("error").is_none(), "{subscribed}");
}
