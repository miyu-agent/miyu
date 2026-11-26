//! 握手、一轮正文和思考、工具步骤、打断、被拒、斜杠命令。

use serde_json::json;

use super::{Editor, home, joined, kind, updates};
use crate::support::outside::Outside;
use miyu_kernel::event::ErrorClass;
use miyu_session::testkit::Play;

#[tokio::test]
async fn the_handshake_offers_what_miyu_has() {
    let home = home([]);
    let (_editor, hello) = Editor::with(&home, json!({})).await;
    let result = &hello["result"];
    assert_eq!(result["protocolVersion"], 1);
    let caps = &result["agentCapabilities"];
    assert_eq!(caps["loadSession"], true);
    assert_eq!(caps["promptCapabilities"]["image"], true);
    assert_eq!(caps["promptCapabilities"]["embeddedContext"], true);
    assert_eq!(
        caps["sessionCapabilities"],
        json!({"list": {}, "resume": {}, "close": {}})
    );
    // 核心没有只对一个会话的 MCP 接法：不声明（设计 73 第五节第 6 条）。
    assert!(caps.get("mcpCapabilities").is_none(), "{caps}");
    assert_eq!(result["authMethods"], json!([]));
    assert_eq!(result["agentInfo"]["name"], "miyu");
}

#[tokio::test]
async fn requests_before_the_handshake_are_refused() {
    let home = home([]);
    let (connection, token) = miyu_ipc::connect(&home.root).await.expect("连得上");
    let relink = Box::new(super::Relink {
        root: home.root.clone(),
        eager: false,
        dropped: std::sync::Arc::default(),
    });
    let (mut to_agent, agent_in) = tokio::io::duplex(1 << 16);
    let (agent_out, from_agent) = tokio::io::duplex(1 << 16);
    let running = tokio::spawn(async move {
        let input = tokio::io::BufReader::new(agent_in);
        miyu_cli::acp_on(
            connection,
            &token,
            relink,
            input,
            agent_out,
            miyu_cli::language::Language::Chinese,
        )
        .await
    });
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
    let line = json!({"jsonrpc": "2.0", "id": 1, "method": "session/new", "params": {"cwd": "/"}});
    to_agent
        .write_all(format!("{line}\n").as_bytes())
        .await
        .expect("写得进");
    let mut lines = tokio::io::BufReader::new(from_agent).lines();
    let reply = crate::support::within("回应", lines.next_line())
        .await
        .expect("读得了")
        .expect("有");
    let reply: serde_json::Value = serde_json::from_str(&reply).expect("JSON");
    assert_eq!(reply["error"]["code"], -32600, "{reply}");
    // 编辑器关了管道：好好退出。
    drop(to_agent);
    assert_eq!(
        crate::support::within("退出", running)
            .await
            .expect("跑完了"),
        0
    );
}

#[tokio::test]
async fn a_new_session_works_in_the_editor_directory_with_the_default_permission() {
    let work = Outside::new();
    let home = home([]);
    let mut editor = Editor::new(&home).await;
    let (_, reply) = editor
        .call("session/new", json!({"cwd": work.text(), "mcpServers": []}))
        .await;
    let result = &reply["result"];
    assert!(result["sessionId"].is_string(), "{reply}");
    // 照 Miyu 的默认：开放权限（项目主人 2026-11-26 定）。
    assert_eq!(result["modes"]["currentModeId"], "full");
    let ids: Vec<&str> = result["modes"]["availableModes"]
        .as_array()
        .expect("有")
        .iter()
        .filter_map(|mode| mode["id"].as_str())
        .collect();
    assert_eq!(ids, ["full", "read-only"]);
    assert_eq!(result["configOptions"][0]["id"], "mode");
    assert_eq!(result["configOptions"][0]["currentValue"], "full");
    // 开好以后报斜杠命令：Miyu 的命令表，加上头自己的 /compact。
    let (_, commands) = editor
        .until(|message| kind(message) == Some("available_commands_update"))
        .await;
    let names: Vec<&str> = commands["params"]["update"]["availableCommands"]
        .as_array()
        .expect("有")
        .iter()
        .filter_map(|command| command["name"].as_str())
        .collect();
    assert!(
        names.contains(&"compact") && names.contains(&"clear"),
        "{names:?}"
    );
    // 会话就是 Miyu 的会话，工作目录是编辑器给的。
    let session = result["sessionId"].as_str().expect("有");
    let log = super::log(&home, session);
    let created = serde_json::to_value(&log[0]).expect("JSON");
    assert_eq!(created["kind"], "session.created", "{created}");
    assert_eq!(created["body"]["cwd"], work.text(), "{created}");
}

#[tokio::test]
async fn a_relative_directory_is_refused() {
    let home = home([]);
    let mut editor = Editor::new(&home).await;
    let (_, reply) = editor
        .call(
            "session/new",
            json!({"cwd": "relative/dir", "mcpServers": []}),
        )
        .await;
    assert_eq!(reply["error"]["code"], -32602, "{reply}");
}

#[tokio::test]
async fn a_turn_streams_its_thinking_and_text_and_ends_the_prompt() {
    let work = Outside::new();
    let home = home([Play::Thinks {
        thinking: "想一想。",
        text: "你好，我是 Miyu。",
    }]);
    let mut editor = Editor::new(&home).await;
    let (session, _) = editor.open(&work.text()).await;
    let (seen, reply) = editor.say(&session, "你好").await;
    assert_eq!(reply["result"]["stopReason"], "end_turn", "{reply}");
    assert_eq!(joined(&seen, "agent_thought_chunk"), "想一想。");
    assert_eq!(joined(&seen, "agent_message_chunk"), "你好，我是 Miyu。");
    // 思考、正文各自一条（`messageId` 不同），编辑器不把它们接在别的块后面。
    let thought = updates(&seen, "agent_thought_chunk")[0]["messageId"].clone();
    let reply = updates(&seen, "agent_message_chunk")[0]["messageId"].clone();
    assert!(
        thought.is_string() && reply.is_string() && thought != reply,
        "{thought} {reply}"
    );
    // 编辑器自己发的那一句不报回去。
    assert_eq!(joined(&seen, "user_message_chunk"), "", "{seen:#?}");
    // 每一条都是这个会话的。
    for message in seen.iter().filter(|message| kind(message).is_some()) {
        assert_eq!(message["params"]["sessionId"], session.as_str());
    }
    // 用量：这个核心的假模型没有窗口，规范要用了多少、窗口多大两个都在，所以不报（报的样子见 `status/tests.rs`）。
    assert!(updates(&seen, "usage_update").is_empty(), "{seen:#?}");
}

#[tokio::test]
async fn tool_steps_carry_titles_kinds_locations_and_diffs() {
    let work = Outside::new();
    let notes = work.file("notes.md", "一\n二\n三\n");
    let notes = notes.to_string_lossy().into_owned();
    let edit = json!({"file_path": notes, "edits": [{"old_string": "二", "new_string": "贰"}]});
    let home = home([
        Play::calls(&[("read", &json!({"file_path": notes}).to_string())]),
        Play::calls(&[("edit", &edit.to_string())]),
        Play::Says("改好了。"),
    ]);
    let mut editor = Editor::new(&home).await;
    let (session, _) = editor.open(&work.text()).await;
    let (seen, reply) = editor.say(&session, "改一下").await;
    assert_eq!(reply["result"]["stopReason"], "end_turn", "{reply}");
    let calls = updates(&seen, "tool_call");
    assert_eq!(calls.len(), 2, "{calls:#?}");
    assert_eq!(calls[0]["kind"], "read");
    assert_eq!(calls[0]["name"], "read");
    // 还在写参数时标题是「准备……」，参数写完换成工具卡片上的那一句。
    let titles: Vec<&str> = updates(&seen, "tool_call_update")
        .into_iter()
        .filter(|update| update["toolCallId"] == calls[0]["toolCallId"])
        .filter_map(|update| update["title"].as_str())
        .collect();
    assert!(
        titles
            .iter()
            .any(|title| title.starts_with("读取 ") && title.ends_with("notes.md · 3 行")),
        "{titles:?}"
    );
    assert_eq!(calls[1]["kind"], "edit");
    let done: Vec<_> = updates(&seen, "tool_call_update")
        .into_iter()
        .filter(|update| update["status"] == "completed")
        .collect();
    assert_eq!(done.len(), 2, "{seen:#?}");
    // 改文件的：带位置，内容是改前、改后的对比。
    let edited = done
        .iter()
        .find(|update| update["toolCallId"] == calls[1]["toolCallId"])
        .expect("编辑那一步做完了");
    let real = std::fs::canonicalize(&notes)
        .expect("在")
        .to_string_lossy()
        .into_owned();
    assert_eq!(edited["locations"][0]["path"], real.as_str(), "{edited}");
    let diff = &edited["content"][0];
    assert_eq!(diff["type"], "diff", "{edited}");
    assert!(
        diff["oldText"]
            .as_str()
            .is_some_and(|old| old.contains("二\n")),
        "{diff}"
    );
    assert!(
        diff["newText"]
            .as_str()
            .is_some_and(|new| new.contains("贰\n")),
        "{diff}"
    );
    assert_eq!(joined(&seen, "agent_message_chunk"), "改好了。");
}

#[tokio::test]
async fn cancel_interrupts_the_turn_and_answers_cancelled() {
    let work = Outside::new();
    let home = home([Play::Holds]);
    let mut editor = Editor::new(&home).await;
    let (session, _) = editor.open(&work.text()).await;
    let params = json!({"sessionId": session, "prompt": [{"type": "text", "text": "等一下"}]});
    let id = editor.send("session/prompt", params).await;
    home.until_a_turn_starts().await;
    editor
        .notify("session/cancel", json!({"sessionId": session}))
        .await;
    let (_, reply) = editor.until_reply(id).await;
    assert_eq!(reply["result"]["stopReason"], "cancelled", "{reply}");
}

#[tokio::test]
async fn a_refusal_ends_with_refusal() {
    let work = Outside::new();
    let home = home([Play::Fails {
        class: ErrorClass::ContentPolicy,
        wait_ms: None,
    }]);
    let mut editor = Editor::new(&home).await;
    let (session, _) = editor.open(&work.text()).await;
    let (_, reply) = editor.say(&session, "写点不该写的").await;
    assert_eq!(reply["result"]["stopReason"], "refusal", "{reply}");
}

#[tokio::test]
async fn a_slash_command_runs_as_miyu_runs_it() {
    let work = Outside::new();
    let home = home([]);
    let mut editor = Editor::new(&home).await;
    let (session, _) = editor.open(&work.text()).await;
    // 头自己的 /compact：没有能压的，照核心的原话说一句，这一句照常结束。
    let (seen, reply) = editor.say(&session, "/compact").await;
    assert_eq!(reply["result"]["stopReason"], "end_turn", "{reply}");
    assert!(
        !joined(&seen, "agent_message_chunk").is_empty(),
        "{seen:#?}"
    );
    // Miyu 的命令：/todo 回一句，不开一轮。
    let (seen, reply) = editor.say(&session, "/todo").await;
    assert_eq!(reply["result"]["stopReason"], "end_turn", "{reply}");
    assert!(
        !joined(&seen, "agent_message_chunk").is_empty(),
        "{seen:#?}"
    );
    // 没开过一轮：日志里没有回合。
    let log = super::log(&home, &session);
    assert!(
        log.iter()
            .all(|event| !matches!(event.body, miyu_kernel::event::Body::TurnStarted(_)))
    );
}
