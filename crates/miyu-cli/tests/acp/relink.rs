//! 核心重启以后（施工 ACP-9）：`miyu acp` 不退出。正在进行的那一轮照出错回；之后的请求照常（核心没连着的当场再连），编辑器里
//! 开着的会话重新订阅了视图流。

use std::sync::Arc;

use serde_json::{Value, json};

use super::{Editor, elsewhere, home, kind};
use crate::support::outside::Outside;
use crate::support::{shipped_tools, within};
use miyu_session::testkit::{Play, Script};

/// 在别处把会话 `session` 切成只读。
async fn read_only_elsewhere(home: &crate::support::Home, session: &str) {
    let params = json!({"session": session, "read_only": true});
    let reply = elsewhere(home, "session.set_permission_level", params).await;
    assert!(reply["result"].is_object(), "{reply}");
}

/// 是会话 `session` 报的当前模式：只读。
fn read_only(message: &Value, session: &str) -> bool {
    kind(message) == Some("current_mode_update")
        && message["params"]["sessionId"] == session
        && message["params"]["update"]["currentModeId"] == "read-only"
}

#[tokio::test]
async fn after_a_restart_the_running_turn_fails_and_the_next_requests_go_on() {
    let work = Outside::new();
    let mut home = home([Play::Holds]);
    // 自己重连的那一次不连（像人停了核心）：等编辑器要用时再连。
    let (mut editor, _) = Editor::relinking(&home, json!({}), false).await;
    let (session, _) = editor.open(&work.text()).await;
    let prompt = json!({"sessionId": session, "prompt": [{"type": "text", "text": "你好"}]});
    let asked = editor.send("session/prompt", prompt).await;
    home.until_a_turn_starts().await;
    // 新的核心：载入时接着干重启时跑到一半的那一轮，再答下一句。
    let plays = [Play::Says("接着说完。"), Play::Says("还在。")];
    home.restart(Arc::new(Script::new(plays)), shipped_tools())
        .await;
    let (_, reply) = editor.until_reply(asked).await;
    assert!(
        reply["error"].is_object() && reply.get("result").is_none(),
        "正在进行的那一轮照出错回：{reply}"
    );
    within("知道断了", editor.dropped.notified()).await;
    // 之后照常：新开一个会话（以前到这里 `miyu acp` 已经退了）。
    let (second, opened) = editor.open(&work.text()).await;
    assert_ne!(second, session, "{opened}");
    assert!(opened["result"]["configOptions"].is_array(), "{opened}");
    // 原来那个会话重新订阅了：在别处切的模式报得到，说得了话。
    read_only_elsewhere(&home, &session).await;
    editor.until(|message| read_only(message, &session)).await;
    let (_, said) = editor.say(&session, "还在吗").await;
    assert_eq!(said["result"]["stopReason"], "end_turn", "{said}");
    assert_eq!(editor.close().await, 0, "编辑器关了管道才退出");
}

#[tokio::test]
async fn after_a_restart_open_sessions_are_followed_again_by_themselves() {
    let work = Outside::new();
    let mut home = home([]);
    let (mut editor, _) = Editor::relinking(&home, json!({}), true).await;
    let (session, _) = editor.open(&work.text()).await;
    home.restart(Arc::new(Script::new([])), shipped_tools())
        .await;
    // 编辑器什么都没发：断了以后自己连上、重新订阅了，在别处切的模式照样报过来。
    read_only_elsewhere(&home, &session).await;
    editor.until(|message| read_only(message, &session)).await;
    assert_eq!(editor.close().await, 0, "编辑器关了管道才退出");
}
