//! 从软件源装（施工 G-4 上，设计 `37-软件源.md` 第四节第 4、5 条）：真核心走一遍，假服务器当软件源，包文件是真打出来的。缺的
//! 依赖一起装、先装依赖；下载进度推给发起的连接；本地库记来源仓库；下好的进缓存，再装不再下；看一眼不下载；哈希对不上的不装；
//! 镜像的 `files/` 在前。

use serde_json::{Value, json};

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_source::{archive, sign};

use crate::support::source::{core, keys, serving};
use crate::support::*;

/// 打一个包：编号、依赖，交回文件名、字节。
fn package(home: &Home, id: &str, depends: &[&str]) -> (String, Vec<u8>) {
    let folder = home.work.join("src").join(id);
    std::fs::create_dir_all(&folder).expect("建得了");
    let depends: Vec<String> = depends.iter().map(|d| format!("\"{d}\"")).collect();
    let mut text = format!("[package]\nversion = \"1.0\"\nname = {{ en = \"{id}\" }}\n");
    if !depends.is_empty() {
        text.push_str(&format!(
            "\n[depends]\npackages = [{}]\n",
            depends.join(", ")
        ));
    }
    std::fs::write(folder.join("package.toml"), text).expect("写得进");
    let name = archive::file_name(id, "1.0", None);
    let out = home.work.join("out").join(&name);
    std::fs::create_dir_all(out.parent().expect("有上一层")).expect("建得了");
    archive::pack(&folder, id, &out).expect("打得成");
    (name, std::fs::read(&out).expect("在"))
}

/// 索引里的一条：照真的包文件写大小和哈希；原址是到不了的，下载走镜像的 `files/`。
fn entry(id: &str, depends: &[&str], file: &(String, Vec<u8>)) -> Value {
    json!({"id": id, "version": "1.0", "name": {"en": id}, "origin": format!("github.com/miyu-agent/{id}"),
           "official": true, "depends": depends,
           "files": [{"url": format!("https://example.invalid/{}", file.0), "size": file.1.len(),
                      "sha256": hex(&file.1)}]})
}

fn hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn signed(entries: Vec<Value>, secret: &str) -> (Vec<u8>, Vec<u8>) {
    let index = serde_json::to_vec(
        &json!({"version": 1, "generated": "2026-10-19T03:00:00Z", "packages": entries}),
    )
    .expect("写得成");
    let signature =
        serde_json::to_vec(&sign::sign(&index, secret).expect("签得了")).expect("写得成");
    (index, signature)
}

fn bytes(body: &[u8]) -> Reply {
    Reply::stream(vec![Piece::Bytes(body.to_vec())])
}

/// 发一条请求，收下回应之前推来的所有通知。
async fn call_collecting(client: &mut Client, method: &str, params: Value) -> (Value, Vec<Value>) {
    let request = json!({"jsonrpc": "2.0", "id": "c", "method": method, "params": params});
    client.line(&request.to_string()).await;
    let mut pushed = Vec::new();
    loop {
        let next = client.next().await.expect("有回应");
        if next.get("id").is_some() {
            return (next, pushed);
        }
        pushed.push(next);
    }
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
        .join("home/alice/packages")
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
