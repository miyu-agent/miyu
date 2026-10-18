//! 拿软件源的索引（施工 G-3 上，设计 `37-软件源.md` 第四节第 1、2 条、第五节）：真核心走一遍，假服务器当软件源，测试自己的
//! 钥匙签。拿到的存进 `state/packages/.sync/`、24 小时以内不再拿、`force` 的照拿；比本机旧的不收、照用本机的、带原因；签名
//! 不对、连不上的试下一处，镜像在前；一处都不成、本机也没有的拒 `source_unreachable`。

use serde_json::{Value, json};

use miyu_http::testkit::Server;

use crate::support::source::{core, keys, serving, signed};
use crate::support::*;

async fn sync(client: &mut Client, force: bool) -> Value {
    client
        .call("s", "package.sync", json!({"force": force}))
        .await
}

#[tokio::test]
async fn a_signed_index_is_kept_and_not_fetched_again_within_a_day() {
    let home = Home::new();
    let (secret, trusted) = keys();
    let first = signed("2026-10-19T03:00:00Z", &["embed", "memes"], &secret);
    let newer = signed("2026-10-20T03:00:00Z", &["embed"], &secret);
    let older = signed("2026-10-18T03:00:00Z", &[], &secret);
    let mut replies = serving(&first);
    replies.extend(serving(&newer));
    replies.extend(serving(&older));
    let server = Server::start(replies).await;
    let mut client = Client::connect(core(&home, &server.base_url, &[], trusted));
    client.hello().await;

    let got = sync(&mut client, false).await;
    assert_eq!(got["result"]["packages"], 2, "{got}");
    assert_eq!(got["result"]["generated"], "2026-10-19T03:00:00Z");
    assert!(got["result"].get("stale").is_none(), "{got}");
    let dir = home.root.path().join("state/packages/.sync");
    assert_eq!(
        std::fs::read(dir.join("index.json")).expect("存了"),
        first.0,
        "照收到的字节存"
    );
    assert!(dir.join("index.json.sig").is_file() && dir.join("fetched").is_file());
    let paths: Vec<String> = server.received().iter().map(|r| r.path.clone()).collect();
    assert_eq!(paths, ["/v1/index.json", "/v1/index.json.sig"]);

    let again = sync(&mut client, false).await;
    assert_eq!(again["result"]["packages"], 2, "{again}");
    assert_eq!(server.received().len(), 2, "24 小时以内不再拿");

    let forced = sync(&mut client, true).await;
    assert_eq!(
        forced["result"]["generated"], "2026-10-20T03:00:00Z",
        "{forced}"
    );
    let rolled = sync(&mut client, true).await;
    assert_eq!(
        rolled["result"]["generated"], "2026-10-20T03:00:00Z",
        "旧的不收：{rolled}"
    );
    assert!(
        rolled["result"]["stale"]
            .as_str()
            .is_some_and(|why| why.contains("older")),
        "{rolled}"
    );
}

#[tokio::test]
async fn a_mirror_is_tried_first_and_a_bad_one_is_passed_over() {
    let home = Home::new();
    let (secret, trusted) = keys();
    let (other, _) = keys();
    let mut replies = serving(&signed("2026-10-19T03:00:00Z", &["embed"], &other));
    replies.extend(serving(&signed(
        "2026-10-19T03:00:00Z",
        &["memes"],
        &secret,
    )));
    let server = Server::start(replies).await;
    // 镜像签错了钥匙；软件源是同一个服务器的另一条路径，照先后排在后面。
    let mirror = format!("{}/mirror", server.base_url);
    let mut client = Client::connect(core(&home, &server.base_url, &[&mirror], trusted));
    client.hello().await;
    let got = sync(&mut client, false).await;
    assert_eq!(got["result"]["packages"], 1, "{got}");
    let paths: Vec<String> = server.received().iter().map(|r| r.path.clone()).collect();
    assert_eq!(
        paths,
        [
            "/v1/mirror/index.json",
            "/v1/mirror/index.json.sig",
            "/v1/index.json",
            "/v1/index.json.sig"
        ],
        "镜像在前"
    );
}

#[tokio::test]
async fn nothing_reachable_and_nothing_kept_is_refused() {
    let home = Home::new();
    let (_, trusted) = keys();
    let (other, _) = keys();
    let server = Server::start(serving(&signed("2026-10-19T03:00:00Z", &[], &other))).await;
    // 数据根里塞一份别的钥匙签的、时刻还新的：本机那份每次读都再验，验不过的当没有。
    let forged = signed("2026-10-19T03:00:00Z", &["evil"], &other);
    let dir = home.root.path().join("state/packages/.sync");
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::write(dir.join("index.json"), &forged.0).expect("写得进");
    std::fs::write(dir.join("index.json.sig"), &forged.1).expect("写得进");
    std::fs::write(dir.join("fetched"), jiff::Timestamp::now().to_string()).expect("写得进");
    let mut client = Client::connect(core(&home, &server.base_url, &[], trusted));
    client.hello().await;
    let got = sync(&mut client, false).await;
    assert_eq!(reason(&got), Some("source_unreachable"), "{got}");
    assert!(
        got["error"]["data"]["why"]
            .as_str()
            .is_some_and(|why| why.contains("unknown key")),
        "{got}"
    );
    assert_eq!(
        std::fs::read(dir.join("index.json")).expect("在"),
        forged.0,
        "什么都没换"
    );
}
