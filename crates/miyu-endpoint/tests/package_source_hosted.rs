//! 从软件源装的包的 `hosted`（施工 RB-1）：模型驱动会让供应商自己跑的工具。索引条目写了的，装之前看一眼、`package.info` 照它原样交
//! （同从包文件夹装的那一份）；以前的索引没有这一格的不出。

use serde_json::{Value, json};

use miyu_http::testkit::Server;

use crate::support::source::{
    call_collecting, core, file_entry, keys, packed, serving, signed_entries,
};
use crate::support::*;

/// 索引里的 `hosted`：驱动 `ag` 会让供应商自己跑两件工具。
fn hosted() -> Value {
    json!({"ag": ["search_web", "read_url_content"]})
}

/// 带一个写了 `hosted` 的包 `gravity`、一个没写的 `plain` 的索引，签好、照它回的软件源，连好的头。
async fn connected(home: &Home) -> (Client, Server) {
    let (secret, trusted) = keys();
    let gravity = packed(home, "gravity", "1.0", &[]);
    let plain = packed(home, "plain", "1.0", &[]);
    let mut entry = file_entry("gravity", "1.0", &[], &gravity);
    entry["hosted"] = hosted();
    let index = signed_entries(
        "2026-11-14T03:00:00Z",
        &[entry, file_entry("plain", "1.0", &[], &plain)],
        &secret,
    );
    let server = Server::start(serving(&index)).await;
    let mut client = Client::connect(core(home, &server.base_url, &[], trusted));
    client.hello().await;
    (client, server)
}

#[tokio::test]
async fn installing_from_the_source_previews_what_its_drivers_have_the_provider_run() {
    let home = Home::new();
    let (mut client, _server) = connected(&home).await;
    let (preview, _) = call_collecting(
        &mut client,
        "package.install",
        json!({"source": "gravity", "preview": true}),
    )
    .await;
    assert_eq!(preview["result"]["hosted"], hosted(), "{preview}");
    let (plain, _) = call_collecting(
        &mut client,
        "package.install",
        json!({"source": "plain", "preview": true}),
    )
    .await;
    assert_eq!(plain["result"]["package"], "plain", "{plain}");
    assert!(
        plain["result"].get("hosted").is_none(),
        "索引没写的不出：{plain}"
    );
}

#[tokio::test]
async fn package_info_from_the_source_hands_over_hosted() {
    let home = Home::new();
    let (mut client, _server) = connected(&home).await;
    let got = client
        .call(
            "i",
            "package.info",
            json!({"package": "gravity", "source": true}),
        )
        .await;
    assert_eq!(got["result"]["hosted"], hosted(), "{got}");
    let plain = client
        .call(
            "j",
            "package.info",
            json!({"package": "plain", "source": true}),
        )
        .await;
    assert!(
        plain["result"].get("hosted").is_none(),
        "索引没写的不出：{plain}"
    );
}
