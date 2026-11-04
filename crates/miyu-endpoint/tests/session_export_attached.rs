//! 导出 Markdown 的第三截（施工 SE-1 中下下，`docs/blueprint/protocol.md`「`session.export`」）：真核心走一遍。
//!
//! - `children: append`：主会话一步派一个（`j1`，它又派一个 `j1.1`），再一步派一个（`j2`）；三个子会话照任务编号接在后面，
//!   回应的 `children` 照同样的先后；子会话里的密钥照样换、`redacted` 合计；`agent` 的开头那一段只写一次，连接是中文旁白
//!   也是英文。
//! - `assets: files`：一张图、一个文件，删掉文件的 blob；图拷进 `<名字>_files/`、Markdown 里写相对路径，文件照占位、回应里
//!   列出来；再导一次、只剩文件夹的都 `export_exists`；配 `inline` 的 `bad_params`；没附件的不建文件夹，文件夹已经有了的照样
//!   `export_exists`。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, Effect, Event};
use miyu_kernel::id::ContentHash;
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;
use miyu_view::markdown::AGENT_NOTE;

use crate::session_export_markdown::markdown;
use crate::support::deleting::*;
use crate::support::*;

/// 密钥的值：派 B 时写在标题里（主会话那一步、B 的小标题各一处），B 的回答里又一处。
const SECRET: &str = "sk-child-abcdef123456";

/// B 的回答。
const LEAKED: &str = "B 查完了，key 是 sk-child-abcdef123456。";

/// 派一个子代理：交代是 `what`（剧本照它分），标题是 `title`。
fn sent(title: &str, what: &str) -> String {
    json!({"description": title, "prompt": what}).to_string()
}

/// 跑一遍：主会话先派 A、再派 B，A 派 A1；等三个子会话各说完一轮。交回客户端和主会话。
async fn family(home: &Home) -> (Client, String) {
    assert!(LEAKED.contains(SECRET));
    let (a, a1) = (sent("查 A", "查 A"), sent("查 A1", "查 A1"));
    let b = sent(&format!("查 B {SECRET}"), "查 B");
    let enough = |text: &'static str| (0..6).map(move |_| Play::Says(text));
    let mut parent = vec![
        Play::calls(&[("subagent", &a)]),
        Play::calls(&[("subagent", &b)]),
    ];
    parent.extend(enough("都派出去了。"));
    let mut child_a = vec![Play::calls(&[("subagent", &a1)])];
    child_a.extend(enough("A 查完了。"));
    let router = Router(Arc::new(vec![
        ("派两个去查", Script::new(parent)),
        ("查 A", Script::new(child_a)),
        ("查 B", Script::new(enough(LEAKED))),
        ("查 A1", Script::new(enough("A1 查完了。"))),
    ]));
    let mut client = Client::connect(home.core_with_models(Arc::new(router), base_tools()));
    client.hello().await;
    let set = client
        .call(
            "k1",
            "secret.set",
            json!({"name": "demo_key", "value": SECRET}),
        )
        .await;
    assert!(set.get("error").is_none(), "{set}");
    let work = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &work).await;
    client.say("s1", &session, "派两个去查").await;
    home.until_turns(&session, 1).await;
    let parent_log = home.log(&session);
    assert_eq!(started_ids(&parent_log), ["j1", "j2"], "两步各派一个");
    until("三个子会话各说完一轮", || {
        let Some(a) = started_child(&parent_log) else {
            return false;
        };
        let a_log = home.log(a.as_str());
        let Some(a1) = started_child(&a_log) else {
            return false;
        };
        let ended = |log: &[Event]| log.iter().any(|e| matches!(e.body, Body::TurnEnded(_)));
        let b = children_of(&parent_log).into_iter().nth(1);
        ended(&a_log) && ended(&home.log(a1.as_str())) && b.is_some_and(|b| ended(&home.log(&b)))
    })
    .await;
    (client, session)
}

/// 日志里派出去的子会话，照先后。
fn children_of(log: &[Event]) -> Vec<String> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(&result.effects),
            _ => None,
        })
        .flatten()
        .filter_map(|effect| match effect {
            Effect::JobStarted(started) => started.session.as_ref().map(|s| s.as_str().to_string()),
            _ => None,
        })
        .collect()
}

/// 放进回应导一份，交回整个回应的 `result`。
async fn inline(client: &mut Client, id: &str, session: &str, params: Value) -> Value {
    let mut params = params;
    params["session"] = json!(session);
    params["to"] = json!("inline");
    let reply = markdown(client, id, params).await;
    assert!(reply.get("error").is_none(), "{reply}");
    reply["result"].clone()
}

#[tokio::test]
async fn appended_children_follow_in_job_order_with_secrets_redacted() {
    let home = Home::new();
    let (mut client, session) = family(&home).await;
    let result = inline(
        &mut client,
        "e1",
        &session,
        json!({"for": "chat", "children": "append"}),
    )
    .await;
    let text = result["text"].as_str().expect("有字");
    let jobs: Vec<&str> = result["children"]
        .as_array()
        .expect("有 children")
        .iter()
        .map(|child| child["job"].as_str().expect("有编号"))
        .collect();
    assert_eq!(jobs, ["j1", "j1.1", "j2"], "照任务编号：{result}");
    assert_eq!(result["children"][0]["title"], json!("查 A"));
    let at = |wanted: &str| {
        text.find(wanted)
            .unwrap_or_else(|| panic!("少了 {wanted:?}：\n{text}"))
    };
    let (a, a1, b) = (
        at("\n\n---\n\n# Subagent j1: 查 A\n\n"),
        at("\n\n---\n\n# Subagent j1.1: 查 A1\n\n"),
        at("\n\n---\n\n# Subagent j2: 查 B [redacted: demo_key]\n\n"),
    );
    assert!(a < a1 && a1 < b, "{text}");
    assert!(text[a..a1].contains("\n\nA 查完了。"), "{text}");
    assert!(text[a1..b].contains("\n\nA1 查完了。"), "{text}");
    assert!(
        text[b..].contains("\n\nB 查完了，key 是 [redacted: demo_key]。\n"),
        "{text}"
    );
    assert!(!text.contains(SECRET), "{text}");
    assert!(
        result["redacted"]["demo_key"].as_u64() >= Some(3),
        "主会话那一步、B 的小标题、B 的回答合计：{result}"
    );
    assert_eq!(text.matches("token 估算").count(), 1, "{text}");

    let agent = inline(
        &mut client,
        "e2",
        &session,
        json!({"for": "agent", "children": "append", "locale": "zh"}),
    )
    .await;
    let agent = agent["text"].as_str().expect("有字");
    assert_eq!(agent.matches(AGENT_NOTE).count(), 1, "{agent}");
    assert!(
        agent.contains(
            "\n\n---\n\n# Subagent j2: 查 B [redacted: demo_key]\n\n- Working directory: "
        ),
        "{agent}"
    );

    let alone = inline(&mut client, "e3", &session, json!({"for": "chat"})).await;
    assert_eq!(alone["children"], json!([]), "不写 append 的不带：{alone}");
    let alone = alone["text"].as_str().expect("有字");
    assert!(!alone.contains("\n# Subagent"), "{alone}");
}

/// 传一个文件，交回 `blob.put` 的回应。
async fn put(client: &mut Client, home: &Home, name: &str, bytes: &[u8]) -> Value {
    let path = home.work.join(name);
    std::fs::write(&path, bytes).expect("写得了");
    let reply = client.call("p", "blob.put", json!({"path": path})).await;
    reply["result"].clone()
}

/// 一张 `width` × `height` 的 PNG 的开头。
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

#[tokio::test]
async fn assets_go_into_a_folder_beside_the_markdown() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([
        Play::Says("看到了。"),
        Play::Says("好。"),
        Play::Says("好。"),
    ])));
    client.hello().await;
    let picture = png(800, 600);
    let image = put(&mut client, &home, "shot one.png", &picture).await;
    let notes = put(&mut client, &home, "notes.md", "# 待办\n".as_bytes()).await;
    let session = client.create("c1", "~").await;
    let reply = client
        .call(
            "c2",
            "session.send",
            json!({"session": session, "text": "看看", "attachments": [image, notes]}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 1).await;
    // 文件的 blob 没了：照占位写、回应里列出来。
    let gone = notes["blob"].as_str().expect("有哈希").to_string();
    let hash: ContentHash = serde_json::from_value(json!(gone)).expect("合写法");
    std::fs::remove_file(Blobs::new(home.root.blobs()).path(&hash)).expect("删得掉");

    let out = home.work.join("talk.md");
    let written = markdown(
        &mut client,
        "e1",
        json!({"session": session, "to": {"path": out}, "assets": "files"}),
    )
    .await;
    let result = &written["result"];
    let folder = home.work.join("talk_files");
    assert_eq!(result["assets"]["dir"], json!(folder), "{written}");
    assert_eq!(result["assets"]["files"], json!(1), "{written}");
    assert_eq!(result["assets"]["missing"], json!([gone]), "{written}");
    assert_eq!(
        std::fs::read(folder.join("shot one.png")).expect("拷过去了"),
        picture
    );
    assert_eq!(std::fs::read_dir(&folder).expect("在").count(), 1);
    let text = std::fs::read_to_string(&out).expect("写出来了");
    assert!(
        text.contains(
            "看看\n\n- ![shot one.png, 800×600](talk_files/shot%20one.png)\n- [文件：notes.md]"
        ),
        "{text}"
    );

    let again = markdown(
        &mut client,
        "e2",
        json!({"session": session, "to": {"path": out}, "assets": "files"}),
    )
    .await;
    assert_eq!(reason(&again), Some("export_exists"), "{again}");
    std::fs::remove_file(&out).expect("删得掉");
    let folder_left = markdown(
        &mut client,
        "e3",
        json!({"session": session, "to": {"path": out}, "assets": "files"}),
    )
    .await;
    assert_eq!(
        reason(&folder_left),
        Some("export_exists"),
        "只剩文件夹也算：{folder_left}"
    );
    assert!(!out.exists(), "什么都不写");
    let inline = markdown(
        &mut client,
        "e4",
        json!({"session": session, "to": "inline", "assets": "files"}),
    )
    .await;
    assert_eq!(reason(&inline), Some("bad_params"), "{inline}");

    let plain = client.create("c3", "~").await;
    client.say("s2", &plain, "没有附件").await;
    home.until_turns(&plain, 1).await;
    let bare = home.work.join("bare.md");
    let written = markdown(
        &mut client,
        "e5",
        json!({"session": plain, "to": {"path": bare}, "assets": "files"}),
    )
    .await;
    assert_eq!(
        written["result"]["assets"],
        json!({"files": 0, "missing": []}),
        "{written}"
    );
    assert!(bare.is_file());
    assert!(!home.work.join("bare_files").exists(), "没附件的不建文件夹");
    // 没附件、文件夹却已经有了的也不导：写之前就查，不等到建文件夹那一刻。
    let other = home.work.join("other.md");
    std::fs::create_dir(home.work.join("other_files")).expect("建得了");
    let blocked = markdown(
        &mut client,
        "e6",
        json!({"session": plain, "to": {"path": other}, "assets": "files"}),
    )
    .await;
    assert_eq!(reason(&blocked), Some("export_exists"), "{blocked}");
    assert!(!other.exists(), "什么都不写");
}
