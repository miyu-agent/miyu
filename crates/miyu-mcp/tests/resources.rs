//! 资源（施工 X-41）：服务声明了 `resources` 的才认，新旧两个时代都认；资源、地址模板一次一页，照 `nextCursor` 拿下一页，每项只留
//! 规范里那几格、没有地址的不收；读回来的照内容收（字、二进制），没有这个资源的照服务的原话交回。

use serde_json::json;

use miyu_mcp::testkit::{Kind, Script, resources, templates};
use miyu_mcp::{Content, Failed};

use crate::support::connected;

/// 新时代、带资源和模板的剧本，一页两项。
fn offering() -> Script {
    Script {
        resources: resources(),
        resource_templates: templates(),
        page: 2,
        ..Script::modern()
    }
}

#[tokio::test]
async fn declared_resources_come_a_page_at_a_time_with_the_listed_fields_only() {
    let (client, fake) = connected(offering()).await;
    assert!(client.offers_resources());
    let first = client.resources(None).await.expect("列得出");
    assert_eq!(
        first.items,
        [
            json!({"uri": "file:///notes.txt", "name": "notes", "title": "Notes", "description": "Things to remember.", "mimeType": "text/plain", "size": 18})
        ],
        "多写的 `_meta` 丢掉，没有地址的不收"
    );
    assert_eq!(first.next.as_deref(), Some("2"));
    let second = client
        .resources(first.next.as_deref())
        .await
        .expect("列得出");
    let uris: Vec<&str> = second
        .items
        .iter()
        .filter_map(|item| item["uri"].as_str())
        .collect();
    assert_eq!(uris, ["file:///dot.png", "file:///blob.bin"]);
    assert_eq!(second.next, None, "最后一页没有下一页");
    let cursors: Vec<_> = fake
        .received()
        .iter()
        .filter(|message| message["method"] == "resources/list")
        .map(|message| message["params"]["cursor"].clone())
        .collect();
    assert_eq!(cursors, [json!(null), json!("2")], "第一页不写 cursor");
}

#[tokio::test]
async fn templates_keep_their_listed_fields() {
    let (client, _fake) = connected(offering()).await;
    let page = client.resource_templates(None).await.expect("列得出");
    assert_eq!(
        page.items,
        [
            json!({"uriTemplate": "file:///logs/{day}.txt", "name": "day log", "description": "The log of one day.", "mimeType": "text/plain"})
        ]
    );
    assert_eq!(page.next, None);
}

#[tokio::test]
async fn reading_gives_the_contents_and_a_missing_one_is_the_servers_words() {
    let (client, _fake) = connected(offering()).await;
    let text = client
        .read_resource("file:///logs/2026-10-21.txt")
        .await
        .expect("读得出");
    assert_eq!(
        text,
        [Content::Resource {
            uri: "file:///logs/2026-10-21.txt".to_string(),
            mime: Some("text/plain".to_string()),
            text: Some("Log of 2026-10-21.".to_string()),
            blob: None,
        }]
    );
    let binary = client
        .read_resource("file:///blob.bin")
        .await
        .expect("读得出");
    assert_eq!(
        binary,
        [Content::Resource {
            uri: "file:///blob.bin".to_string(),
            mime: Some("application/octet-stream".to_string()),
            text: None,
            blob: Some("AAECAw==".to_string()),
        }]
    );
    let missing = client.read_resource("file:///nope").await;
    assert!(
        matches!(&missing, Err(Failed::Rpc { code: -32002, message, .. }) if message == "Resource not found"),
        "{missing:?}"
    );
}

#[tokio::test]
async fn only_a_declared_server_offers_resources_in_either_era() {
    let (plain, _fake) = connected(Script::modern()).await;
    assert!(!plain.offers_resources(), "没声明的不认");
    let (legacy, _fake) = connected(Script {
        resources: resources(),
        ..Script::legacy(Kind::LegacyRefuses)
    })
    .await;
    assert!(legacy.offers_resources(), "旧时代握手里声明的也认");
}
