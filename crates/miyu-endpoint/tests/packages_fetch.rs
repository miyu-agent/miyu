//! 从软件源装（施工 G-4 上，设计 `37-软件源.md` 第四节第 4、5 条）：真核心走一遍，假服务器当软件源，包文件是真打出来的。缺的
//! 依赖一起装、先装依赖；下载进度推给发起的连接；本地库记来源仓库；下好的进缓存，再装不再下；看一眼不下载；哈希对不上的不装；
//! 镜像的 `files/` 在前。

use serde_json::{Value, json};

use miyu_http::testkit::Server;

use crate::support::source::{
    bytes, call_collecting, core, file_entry, keys, packed, serving, signed_entries,
};
use crate::support::*;

/// 打一个包：编号、依赖，版本 1.0。
fn package(home: &Home, id: &str, depends: &[&str]) -> (String, Vec<u8>) {
    packed(home, id, "1.0", depends)
}

/// 索引里的一条，版本 1.0。
fn entry(id: &str, depends: &[&str], file: &(String, Vec<u8>)) -> Value {
    file_entry(id, "1.0", depends, file)
}

fn signed(entries: Vec<Value>, secret: &str) -> (Vec<u8>, Vec<u8>) {
    signed_entries("2026-10-19T03:00:00Z", &entries, secret)
}

fn desc(home: &Home, id: &str) -> Value {
    let text = std::fs::read_to_string(
        home.root
            .path()
            .join("state/packages/.local")
            .join(id)
            .join("desc"),
    )
    .expect("记了");
    serde_json::from_str(&text).expect("是 JSON")
}

fn installed(home: &Home, id: &str) -> bool {
    home.root
        .path()
        .join("packages")
        .join(id)
        .join("package.toml")
        .is_file()
}

#[tokio::test]
async fn a_package_comes_with_its_dependency_and_is_recorded_with_its_origin() {
    let home = Home::new();
    let (secret, trusted) = keys();
    let app = package(&home, "app", &["lib"]);
    let lib = package(&home, "lib", &[]);
    let pair = signed(
        vec![entry("app", &["lib"], &app), entry("lib", &[], &lib)],
        &secret,
    );
    // 服务器照连接的先后回：索引、签名（看一眼），lib、app（真装）。镜像在前，下载都走它的 `files/`。
    let mut replies = serving(&pair);
    replies.extend([bytes(&lib.1), bytes(&app.1)]);
    let server = Server::start(replies).await;
    let mirror = format!("{}/m", server.base_url);
    let mut client = Client::connect(core(
        &home,
        "https://unreachable.invalid/",
        &[&mirror],
        trusted,
    ));
    client.hello().await;

    let (preview, _) = call_collecting(
        &mut client,
        "package.install",
        json!({"source": "app", "preview": true}),
    )
    .await;
    let preview = &preview["result"];
    assert_eq!(preview["package"], "app", "{preview}");
    assert_eq!(
        preview["with"],
        json!([{"package": "lib", "version": "1.0"}])
    );
    assert_eq!(preview["download"], app.1.len() + lib.1.len());
    assert_eq!(preview["origin"], "github.com/miyu-agent/app");
    assert!(!installed(&home, "app"), "看一眼什么都不装");

    let (reply, pushed) =
        call_collecting(&mut client, "package.install", json!({"source": "app"})).await;
    assert_eq!(reply["result"]["package"], "app", "{reply}");
    assert_eq!(reply["result"]["with"], json!(["lib"]));
    assert!(installed(&home, "app") && installed(&home, "lib"));
    assert_eq!(desc(&home, "app")["origin"], "github.com/miyu-agent/app");
    assert_eq!(desc(&home, "lib")["origin"], "github.com/miyu-agent/lib");
    let paths: Vec<String> = server.received().iter().map(|r| r.path.clone()).collect();
    assert_eq!(
        paths,
        [
            "/v1/m/index.json",
            "/v1/m/index.json.sig",
            "/v1/m/files/lib-1.0.miyu",
            "/v1/m/files/app-1.0.miyu"
        ],
        "先下依赖，镜像在前"
    );
    let progress: Vec<&Value> = pushed
        .iter()
        .filter(|p| p["method"] == "package.progress")
        .collect();
    let ends: Vec<(String, u64)> = progress
        .iter()
        .filter(|p| p["params"]["done"] == p["params"]["total"])
        .map(|p| {
            (
                p["params"]["package"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                p["params"]["total"].as_u64().unwrap_or(0),
            )
        })
        .collect();
    assert_eq!(
        ends,
        [
            ("lib".to_string(), lib.1.len() as u64),
            ("app".to_string(), app.1.len() as u64)
        ],
        "{pushed:?}"
    );
    assert!(
        home.root
            .path()
            .join("state/packages/.cache/app-1.0.miyu")
            .is_file(),
        "下好的进缓存"
    );

    // 卸了再装（只写 `package`：不是出厂包的照软件源装）：缓存里有、哈希对得上的不再下（服务器已经没有回应可给了）。
    let removed = client
        .call("r", "package.remove", json!({"package": "app"}))
        .await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    let (again, _) =
        call_collecting(&mut client, "package.install", json!({"package": "app"})).await;
    assert_eq!(again["result"]["package"], "app", "{again}");
    assert_eq!(server.received().len(), 4, "没再下");
    assert!(
        again["result"].get("with").is_none(),
        "装着的依赖不再装：{again}"
    );

    // 清缓存：装着那一版的留下，别的删掉。
    std::fs::write(
        home.root.path().join("state/packages/.cache/app-0.9.miyu"),
        b"old",
    )
    .expect("写得进");
    let cleaned = client.call("k", "package.clean", json!({})).await;
    assert_eq!(
        cleaned["result"],
        json!({"removed": 1, "freed": 3}),
        "{cleaned}"
    );
    assert!(
        home.root
            .path()
            .join("state/packages/.cache/app-1.0.miyu")
            .is_file()
    );
}

#[tokio::test]
async fn a_file_that_does_not_match_the_index_is_not_installed() {
    let home = Home::new();
    let (secret, trusted) = keys();
    let app = package(&home, "app", &[]);
    let pair = signed(vec![entry("app", &[], &app)], &secret);
    let mut forged = app.1.clone();
    let last = forged.len() - 1;
    forged[last] ^= 1;
    let mut replies = serving(&pair);
    let mut longer = app.1.clone();
    longer.extend_from_slice(b"more");
    let shorter = app.1[..app.1.len() - 1].to_vec();
    replies.extend([bytes(&forged), bytes(&longer), bytes(&shorter)]);
    let server = Server::start(replies).await;
    let mirror = format!("{}/m", server.base_url);
    let mut client = Client::connect(core(
        &home,
        "https://unreachable.invalid/",
        &[&mirror],
        trusted,
    ));
    client.hello().await;
    let (reply, _) =
        call_collecting(&mut client, "package.install", json!({"source": "app"})).await;
    assert_eq!(reason(&reply), Some("download_failed"), "{reply}");
    assert!(
        reply["error"]["data"]["why"]
            .as_str()
            .is_some_and(|why| why.contains("SHA-256")),
        "{reply}"
    );
    assert!(!installed(&home, "app"));
    assert!(
        !home
            .root
            .path()
            .join("state/packages/.cache/app-1.0.miyu")
            .exists(),
        "坏的不进缓存"
    );
    for said in ["larger than", "bytes, the index says"] {
        let (reply, _) =
            call_collecting(&mut client, "package.install", json!({"source": "app"})).await;
        assert!(
            reply["error"]["data"]["why"]
                .as_str()
                .is_some_and(|why| why.contains(said)),
            "大小不对的先照大小说：{reply}"
        );
    }
    let (unknown, _) =
        call_collecting(&mut client, "package.install", json!({"source": "nothing"})).await;
    assert_eq!(reason(&unknown), Some("unknown_package"));
}
