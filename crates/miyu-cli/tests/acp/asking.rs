//! 确认、提问：核心要问人时翻成编辑器的 `session/request_permission`、`elicitation/create`，编辑器选的照着回核心。

use serde_json::{Value, json};

use super::{Editor, home};
use crate::support::outside::Outside;
use miyu_kernel::event::Body;
use miyu_session::testkit::Play;

/// 问一组题：一道单选，一道没有选项。
fn questions() -> Play {
    let args = json!({"questions": [
        {"question": "用哪个？", "options": [{"label": "甲"}, {"label": "乙"}]},
        {"question": "叫什么？"},
    ]});
    Play::calls(&[("ask_user", &args.to_string())])
}

/// 会话日志里那一次提问的回答（`question.answered` 的 `answers`）。
fn answered(home: &crate::support::Home, session: &str) -> Value {
    let log = super::log(home, session);
    let event = log
        .iter()
        .find(|event| matches!(event.body, Body::QuestionAnswered(_)))
        .expect("答了");
    serde_json::to_value(event).expect("JSON")["body"]["answers"].clone()
}

/// 调一次 `shell` 跑 `git reset --hard`：高危，开放权限下也要确认（施工 5-17、LV-1）。
#[cfg(unix)]
fn reset() -> Play {
    let args = json!({"command": "git reset --hard", "description": "Reset"});
    Play::calls(&[("shell", &args.to_string())])
}

#[cfg(unix)]
// Windows 上 `shell` 是 PowerShell，高危命令只认 bash、zsh 的写法（施工 5-17），`git reset --hard` 不会要确认：只在 Unix 上跑。
#[tokio::test]
async fn an_approval_becomes_a_permission_request_and_the_choice_goes_back() {
    let work = Outside::new();
    let home = home([reset(), Play::Says("没跑。")]);
    let mut editor = Editor::new(&home).await;
    let (session, _) = editor.open(&work.text()).await;
    let params = json!({"sessionId": session, "prompt": [{"type": "text", "text": "重置一下"}]});
    let id = editor.send("session/prompt", params).await;
    let (seen, asked) = editor
        .until(|message| message["method"] == "session/request_permission")
        .await;
    let call = super::updates(&seen, "tool_call");
    let call = call.last().expect("先报了这一步");
    assert_eq!(call["kind"], "execute");
    let params = &asked["params"];
    assert_eq!(params["sessionId"], session.as_str());
    assert_eq!(params["toolCall"]["toolCallId"], call["toolCallId"]);
    assert_eq!(
        params["toolCall"]["rawInput"]["command"],
        "git reset --hard"
    );
    let kinds: Vec<&str> = params["options"]
        .as_array()
        .expect("有")
        .iter()
        .filter_map(|option| option["kind"].as_str())
        .collect();
    // 这一类确认没有放行范围：只有允许这一次、拒绝。
    assert_eq!(kinds, ["allow_once", "reject_once"]);
    let reject = params["options"][1]["optionId"].clone();
    editor
        .reply(
            &asked["id"],
            json!({"outcome": {"outcome": "selected", "optionId": reject}}),
        )
        .await;
    let (seen, reply) = editor.until_reply(id).await;
    assert_eq!(reply["result"]["stopReason"], "end_turn", "{reply}");
    let failed = super::updates(&seen, "tool_call_update")
        .into_iter()
        .any(|update| update["toolCallId"] == call["toolCallId"] && update["status"] == "failed");
    assert!(failed, "拒了的那一步没做成：{seen:#?}");
    let log = super::log(&home, &session);
    let decided = log
        .iter()
        .filter_map(|event| serde_json::to_value(event).ok())
        .find(|event| event["kind"] == "tool.approval_decided")
        .expect("回了核心");
    assert_eq!(decided["body"]["decision"], "deny", "{decided}");
}

#[tokio::test]
async fn questions_go_one_by_one_without_forms() {
    let work = Outside::new();
    let home = home([questions(), Play::Says("好。")]);
    let mut editor = Editor::new(&home).await;
    let (session, _) = editor.open(&work.text()).await;
    let params = json!({"sessionId": session, "prompt": [{"type": "text", "text": "问我"}]});
    let id = editor.send("session/prompt", params).await;
    let (_, asked) = editor
        .until(|message| message["method"] == "session/request_permission")
        .await;
    let params = &asked["params"];
    assert_eq!(params["toolCall"]["title"], "用哪个？");
    let names: Vec<&str> = params["options"]
        .as_array()
        .expect("有")
        .iter()
        .filter_map(|option| option["name"].as_str())
        .collect();
    assert_eq!(names, ["甲", "乙", "跳过"]);
    let second = params["options"][1]["optionId"].clone();
    editor
        .reply(
            &asked["id"],
            json!({"outcome": {"outcome": "selected", "optionId": second}}),
        )
        .await;
    // 第二道没有选项，问不了：当没答，整组交回核心。
    let (_, reply) = editor.until_reply(id).await;
    assert_eq!(reply["result"]["stopReason"], "end_turn", "{reply}");
    assert_eq!(answered(&home, &session), json!([{"picked": ["乙"]}, {}]));
}

#[tokio::test]
async fn questions_go_as_one_form_when_the_editor_has_forms() {
    let work = Outside::new();
    let home = home([questions(), Play::Says("好。")]);
    let (mut editor, _) = Editor::with(&home, json!({"elicitation": {"form": {}}})).await;
    let (session, _) = editor.open(&work.text()).await;
    let params = json!({"sessionId": session, "prompt": [{"type": "text", "text": "问我"}]});
    let id = editor.send("session/prompt", params).await;
    let (_, asked) = editor
        .until(|message| message["method"] == "elicitation/create")
        .await;
    let params = &asked["params"];
    assert_eq!(params["mode"], "form");
    let fields = &params["requestedSchema"]["properties"];
    assert_eq!(fields["q1"]["enum"], json!(["甲", "乙"]));
    assert_eq!(fields["q2"]["type"], "string");
    let content = json!({"q1": "甲", "q1_other": "  ", "q2": "小美"});
    editor
        .reply(
            &asked["id"],
            json!({"action": "accept", "content": content}),
        )
        .await;
    let (_, reply) = editor.until_reply(id).await;
    assert_eq!(reply["result"]["stopReason"], "end_turn", "{reply}");
    assert_eq!(
        answered(&home, &session),
        json!([{"picked": ["甲"]}, {"text": "小美"}])
    );
}
