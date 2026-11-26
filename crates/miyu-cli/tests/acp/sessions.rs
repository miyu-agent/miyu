//! 载入（重放历史）、接上、列出、关掉、切模式、换配置项。

use serde_json::json;

use super::{Editor, elsewhere, home, joined, kind, updates};
use crate::support::outside::Outside;
use miyu_session::testkit::Play;

#[tokio::test]
async fn load_replays_the_history_before_answering() {
    let work = Outside::new();
    let notes = work.file("notes.md", "一\n");
    let read = json!({"file_path": notes.to_string_lossy()}).to_string();
    let home = home([
        Play::calls(&[("read", &read)]),
        Play::Says("读完了。"),
        Play::Says("第二句的回答。"),
    ]);
    let session = {
        let mut editor = Editor::new(&home).await;
        let (session, _) = editor.open(&work.text()).await;
        editor.say(&session, "读一下").await;
        editor.say(&session, "第二句").await;
        session
    };
    // 另一个编辑器（重开了的）载入它：历史照先后报完了才回。
    let mut editor = Editor::new(&home).await;
    let params = json!({"sessionId": session, "cwd": work.text(), "mcpServers": []});
    let (seen, reply) = editor.call("session/load", params).await;
    assert!(reply["result"].is_object(), "{reply}");
    assert_eq!(reply["result"]["modes"]["currentModeId"], "full");
    assert_eq!(joined(&seen, "user_message_chunk"), "读一下第二句");
    assert_eq!(
        joined(&seen, "agent_message_chunk"),
        "读完了。第二句的回答。"
    );
    let calls = updates(&seen, "tool_call");
    assert_eq!(calls.len(), 1, "{seen:#?}");
    assert_eq!(calls[0]["status"], "completed");
    // 先后：你说的、她读的、她答的，再是下一句。
    let order: Vec<&str> = seen
        .iter()
        .filter_map(kind)
        .filter(|kind| {
            matches!(
                *kind,
                "user_message_chunk" | "agent_message_chunk" | "tool_call"
            )
        })
        .collect();
    assert_eq!(
        order,
        [
            "user_message_chunk",
            "tool_call",
            "agent_message_chunk",
            "user_message_chunk",
            "agent_message_chunk"
        ]
    );
    // 接着说：同一个会话。
    let (_, commands) = editor
        .until(|message| kind(message) == Some("available_commands_update"))
        .await;
    assert_eq!(commands["params"]["sessionId"], session.as_str());
}

#[tokio::test]
async fn load_pages_back_through_a_long_history() {
    // 订阅回应只带最新一页（20 轮）：再早的照 `view.page` 往前翻，照先后接起来。
    const ANSWERS: [&str; 22] = [
        "1。", "2。", "3。", "4。", "5。", "6。", "7。", "8。", "9。", "10。", "11。", "12。",
        "13。", "14。", "15。", "16。", "17。", "18。", "19。", "20。", "21。", "22。",
    ];
    let work = Outside::new();
    let home = home(ANSWERS.map(Play::Says));
    let session = {
        let mut editor = Editor::new(&home).await;
        let (session, _) = editor.open(&work.text()).await;
        for _ in ANSWERS {
            editor.say(&session, "下一个").await;
        }
        session
    };
    let mut editor = Editor::new(&home).await;
    let params = json!({"sessionId": session, "cwd": work.text(), "mcpServers": []});
    let (seen, _) = editor.call("session/load", params).await;
    assert_eq!(joined(&seen, "agent_message_chunk"), ANSWERS.concat());
}

#[tokio::test]
async fn resume_attaches_without_replaying_and_goes_on() {
    let work = Outside::new();
    let home = home([Play::Says("一。"), Play::Says("二。")]);
    let session = {
        let mut editor = Editor::new(&home).await;
        let (session, _) = editor.open(&work.text()).await;
        editor.say(&session, "第一句").await;
        session
    };
    let mut editor = Editor::new(&home).await;
    let params = json!({"sessionId": session, "cwd": work.text(), "mcpServers": []});
    let (seen, reply) = editor.call("session/resume", params).await;
    assert!(reply["result"].is_object(), "{reply}");
    assert!(
        updates(&seen, "agent_message_chunk").is_empty(),
        "{seen:#?}"
    );
    assert!(updates(&seen, "user_message_chunk").is_empty(), "{seen:#?}");
    let (seen, reply) = editor.say(&session, "第二句").await;
    assert_eq!(reply["result"]["stopReason"], "end_turn");
    assert_eq!(joined(&seen, "agent_message_chunk"), "二。");
}

#[tokio::test]
async fn an_unknown_session_is_not_found() {
    let work = Outside::new();
    let home = home([]);
    let mut editor = Editor::new(&home).await;
    let missing = "01a14aca-d2c4-768c-acc6-a67e35106d22";
    let params = json!({"sessionId": missing, "cwd": work.text(), "mcpServers": []});
    let (_, reply) = editor.call("session/load", params).await;
    assert_eq!(reply["error"]["code"], -32002, "{reply}");
    assert_eq!(reply["error"]["data"]["sessionId"], missing);
    let params = json!({"sessionId": missing, "prompt": [{"type": "text", "text": "在吗"}]});
    let (_, reply) = editor.call("session/prompt", params).await;
    assert_eq!(reply["error"]["code"], -32002, "{reply}");
}

#[tokio::test]
async fn list_shows_the_sessions_of_the_directory() {
    let here = Outside::new();
    let there = Outside::new();
    let home = home([Play::Says("在。")]);
    let mut editor = Editor::new(&home).await;
    let (session, _) = editor.open(&here.text()).await;
    editor.say(&session, "在吗").await;
    let (other, _) = editor.open(&there.text()).await;
    let (_, reply) = editor
        .call("session/list", json!({"cwd": here.text()}))
        .await;
    let listed = reply["result"]["sessions"].as_array().expect("有").clone();
    assert_eq!(listed.len(), 1, "{reply}");
    assert_eq!(listed[0]["sessionId"], session.as_str());
    assert_eq!(listed[0]["cwd"], here.text());
    assert!(listed[0]["updatedAt"].is_string());
    assert_eq!(listed[0]["title"], "在吗", "没起标题的照第一句");
    // 不写目录的全都列。
    let (_, reply) = editor.call("session/list", json!({})).await;
    let ids: Vec<&str> = reply["result"]["sessions"]
        .as_array()
        .expect("有")
        .iter()
        .filter_map(|item| item["sessionId"].as_str())
        .collect();
    assert!(
        ids.contains(&session.as_str()) && ids.contains(&other.as_str()),
        "{ids:?}"
    );
}

#[tokio::test]
async fn set_mode_switches_the_permission_and_reports_it() {
    let work = Outside::new();
    let home = home([]);
    let mut editor = Editor::new(&home).await;
    let (session, _) = editor.open(&work.text()).await;
    let (_, reply) = editor
        .call(
            "session/set_mode",
            json!({"sessionId": session, "modeId": "read-only"}),
        )
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    // 会话真的切成了只读。
    let log = super::log(&home, &session);
    let changed = log
        .iter()
        .filter_map(|event| serde_json::to_value(event).ok())
        .find(|event| event["kind"] == "session.policy_changed")
        .expect("切了权限");
    assert_eq!(
        changed["body"]["permission"]["read_only"], true,
        "{changed}"
    );
    // 配置项那一套也能切：回整份配置项。
    let params = json!({"sessionId": session, "configId": "mode", "value": "full"});
    let (_, reply) = editor.call("session/set_config_option", params).await;
    assert_eq!(
        reply["result"]["configOptions"][0]["currentValue"], "full",
        "{reply}"
    );
    // 认不得的模式、配置项：-32602。
    let (_, reply) = editor
        .call(
            "session/set_mode",
            json!({"sessionId": session, "modeId": "yolo"}),
        )
        .await;
    assert_eq!(reply["error"]["code"], -32602, "{reply}");
    let params = json!({"sessionId": session, "configId": "colour", "value": "red"});
    let (_, reply) = editor.call("session/set_config_option", params).await;
    assert_eq!(reply["error"]["code"], -32602, "{reply}");
}

#[tokio::test]
async fn close_forgets_the_session_but_miyu_keeps_it() {
    let work = Outside::new();
    let home = home([]);
    let mut editor = Editor::new(&home).await;
    let (session, _) = editor.open(&work.text()).await;
    let (_, reply) = editor
        .call("session/close", json!({"sessionId": session}))
        .await;
    assert_eq!(reply["result"], json!({}));
    let params = json!({"sessionId": session, "prompt": [{"type": "text", "text": "在吗"}]});
    let (_, reply) = editor.call("session/prompt", params).await;
    assert_eq!(
        reply["error"]["code"], -32002,
        "关了的不在这个编辑器里：{reply}"
    );
    // 关两次照样回 {}。
    let (_, reply) = editor
        .call("session/close", json!({"sessionId": session}))
        .await;
    assert_eq!(reply["result"], json!({}));
    assert!(home.sessions().iter().any(|id| id.to_string() == session));
    // 编辑器关了管道：好好退出。
    assert_eq!(editor.close().await, 0);
}

#[tokio::test]
async fn a_mode_switched_elsewhere_is_reported() {
    // 在终端界面里切的：编辑器收到当前的模式和整份配置项（设计 73 第五节第 5 条）。
    let work = Outside::new();
    let home = home([]);
    let mut editor = Editor::new(&home).await;
    let (session, _) = editor.open(&work.text()).await;
    let params = json!({"session": session, "read_only": true});
    let reply = elsewhere(&home, "session.set_permission_level", params).await;
    assert!(reply["result"].is_object(), "{reply}");
    let (_, mode) = editor
        .until(|message| kind(message) == Some("current_mode_update"))
        .await;
    assert_eq!(mode["params"]["update"]["currentModeId"], "read-only");
    let (_, options) = editor
        .until(|message| kind(message) == Some("config_option_update"))
        .await;
    assert_eq!(
        options["params"]["update"]["configOptions"][0]["currentValue"],
        "read-only"
    );
}
