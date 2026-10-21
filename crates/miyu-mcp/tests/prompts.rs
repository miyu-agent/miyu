//! 提示词（施工 X-39）：服务声明了 `prompts` 的才认；翻页列全、照先后，写法不对的不收、说哪一项，新时代的 `ttlMs`；取一个照参数
//! 回几条消息（文字、图、她说的），服务回错照原样交回；旧时代握手里声明的也认；服务说提示词变了的和工具变了同一个数。

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::json;

use miyu_mcp::testkit::{Kind, Script, prompts};
use miyu_mcp::{Content, Failed, Role};

use crate::support::connected;

/// 新时代、带提示词的剧本。
fn offering() -> Script {
    Script {
        prompts: prompts(),
        ..Script::modern()
    }
}

#[tokio::test]
async fn declared_prompts_are_listed_page_by_page() {
    let script = Script {
        page: 1,
        ttl_ms: Some(60_000),
        ..offering()
    };
    let (client, fake) = connected(script).await;
    assert!(client.offers_prompts());
    let listed = client.prompts().await.expect("列得出");
    let names: Vec<&str> = listed
        .prompts
        .iter()
        .map(|prompt| prompt.name.as_str())
        .collect();
    assert_eq!(names, ["review", "hello"]);
    let review = &listed.prompts[0];
    assert_eq!(
        (review.title.as_deref(), review.description.as_deref()),
        (Some("Review code"), Some("Review a piece of code."))
    );
    let arguments: Vec<(&str, bool)> = review
        .arguments
        .iter()
        .map(|argument| (argument.name.as_str(), argument.required))
        .collect();
    assert_eq!(arguments, [("code", true), ("focus", false)]);
    assert!(listed.prompts[1].arguments.is_empty());
    assert_eq!(
        listed.skipped.len(),
        1,
        "没有名字的不收：{:?}",
        listed.skipped
    );
    assert_eq!(listed.fresh_for, Some(Duration::from_secs(60)));
    let cursors: Vec<_> = fake
        .received()
        .iter()
        .filter(|message| message["method"] == "prompts/list")
        .map(|message| message["params"]["cursor"].clone())
        .collect();
    assert_eq!(cursors, [json!(null), json!("1"), json!("2")]);
}

#[tokio::test]
async fn a_prompt_comes_back_as_messages() {
    let (client, fake) = connected(offering()).await;
    let arguments = BTreeMap::from([
        ("code".to_string(), "x = 1".to_string()),
        ("focus".to_string(), "names".to_string()),
    ]);
    let messages = client.prompt("review", &arguments).await.expect("取得到");
    let got: Vec<(Role, Content)> = messages
        .into_iter()
        .map(|message| (message.role, message.content))
        .collect();
    assert_eq!(
        got,
        [
            (
                Role::User,
                Content::Text("Review this code for names:\nx = 1".to_string())
            ),
            (
                Role::User,
                Content::Image {
                    data: "aGk=".to_string(),
                    mime: "image/png".to_string()
                }
            ),
            (Role::Assistant, Content::Text("Sure.".to_string())),
        ]
    );
    let asked = fake
        .received()
        .into_iter()
        .find(|message| message["method"] == "prompts/get")
        .expect("问了");
    assert_eq!(
        asked["params"]["arguments"],
        json!({"code": "x = 1", "focus": "names"})
    );
    assert!(
        asked["params"]["_meta"].is_object(),
        "新时代的请求都带 _meta：{asked}"
    );
}

#[tokio::test]
async fn a_refused_prompt_says_why() {
    let (client, _) = connected(offering()).await;
    let missing = client.prompt("review", &BTreeMap::new()).await;
    assert!(
        matches!(&missing, Err(Failed::Rpc { code: -32602, message, .. }) if message.contains("code")),
        "{missing:?}"
    );
    let unknown = client.prompt("nope", &BTreeMap::new()).await;
    assert!(
        matches!(unknown, Err(Failed::Rpc { code: -32602, .. })),
        "{unknown:?}"
    );
}

#[tokio::test]
async fn without_the_capability_there_are_no_prompts() {
    let (client, _) = connected(Script::modern()).await;
    assert!(!client.offers_prompts());
}

#[tokio::test]
async fn a_legacy_server_declares_prompts_in_its_handshake() {
    let script = Script {
        prompts: prompts(),
        ..Script::legacy(Kind::LegacyRefuses)
    };
    let (client, _) = connected(script).await;
    assert!(client.offers_prompts());
    let listed = client.prompts().await.expect("列得出");
    assert_eq!(listed.prompts.len(), 2);
    assert_eq!(listed.fresh_for, None);
}

#[tokio::test]
async fn a_prompt_change_is_counted_with_tool_changes() {
    let script = Script {
        change_prompts_after_list: true,
        ..Script {
            prompts: prompts(),
            ..Script::legacy(Kind::LegacyRefuses)
        }
    };
    let (client, _) = connected(script).await;
    let mut changes = client.changes();
    assert_eq!(*changes.borrow(), 0);
    client.prompts().await.expect("列得出");
    tokio::time::timeout(
        Duration::from_secs(10),
        changes.wait_for(|count| *count == 1),
    )
    .await
    .expect("十秒内")
    .expect("连接在");
}
