//! 升级（施工 G-5，设计 `37-软件源.md` 第四节第 6、7 条）：真核心走一遍。从软件源装了 1.0，索引换成 2.0：盘点出能升的；看一眼
//! 说下多大；升了版本换成 2.0、来源照旧；从文件夹装的不碰；`hold` 的跳过；软件源里没了的报一句。

use serde_json::json;

use miyu_http::testkit::Server;

use crate::support::source::{
    bytes, call_collecting, core_holding, file_entry, keys, packed, serving, signed_entries,
};
use crate::support::*;

#[tokio::test]
async fn an_installed_package_is_upgraded_and_local_ones_are_left_alone() {
    let home = Home::new();
    let (secret, trusted) = keys();
    let app1 = packed(&home, "app", "1.0", &[]);
    let app2 = packed(&home, "app", "2.0", &[]);
    let kept1 = packed(&home, "kept", "1.0", &[]);
    let first = signed_entries(
        "2026-10-19T03:00:00Z",
        &[
            file_entry("app", "1.0", &[], &app1),
            file_entry("kept", "1.0", &[], &kept1),
        ],
        &secret,
    );
    let second = signed_entries(
        "2026-10-20T03:00:00Z",
        &[
            file_entry("app", "2.0", &[], &app2),
            file_entry("local", "9.0", &[], &app2),
        ],
        &secret,
    );
    // 先后：索引、签名、app 1.0、kept 1.0；拿新索引、签名；app 2.0。
    let mut replies = serving(&first);
    replies.extend([bytes(&app1.1), bytes(&kept1.1)]);
    replies.extend(serving(&second));
    replies.push(bytes(&app2.1));
    let server = Server::start(replies).await;
    let mirror = format!("{}/m", server.base_url);
    let mut client = Client::connect(core_holding(
        &home,
        "https://unreachable.invalid/",
        &[&mirror],
        trusted,
        &["kept"],
    ));
    client.hello().await;
    for id in ["app", "kept"] {
        let (reply, _) =
            call_collecting(&mut client, "package.install", json!({"source": id})).await;
        assert_eq!(reply["result"]["package"], id, "{reply}");
    }
    // 从文件夹装的 local：软件源里有更新的也不碰。
    let folder = home.work.join("local");
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        "[package]\nversion = \"1.0\"\nname = { en = \"Local\" }\n",
    )
    .expect("写得进");
    let local = client
        .call("l", "package.install", json!({"path": folder}))
        .await;
    assert_eq!(local["result"]["package"], "local", "{local}");

    let synced = client
        .call("s", "package.sync", json!({"force": true}))
        .await;
    assert_eq!(
        synced["result"]["generated"], "2026-10-20T03:00:00Z",
        "{synced}"
    );
    let survey = client.call("u", "package.upgrades", json!({})).await;
    let survey = &survey["result"];
    assert_eq!(
        survey["upgrades"],
        json!([{"package": "app", "from": "1.0", "to": "2.0"}]),
        "{survey}"
    );
    assert_eq!(
        survey["gone"],
        json!(["kept"]),
        "软件源里没了的照用、报一句"
    );
    assert_eq!(survey["held"], json!([]), "没了的不算 hold");

    let preview = client
        .call("p", "package.upgrade", json!({"preview": true}))
        .await;
    assert_eq!(preview["result"]["download"], app2.1.len(), "{preview}");
    let (only, _) = call_collecting(
        &mut client,
        "package.upgrade",
        json!({"packages": ["kept"]}),
    )
    .await;
    assert_eq!(
        only["result"]["upgraded"],
        json!([]),
        "只升列着的、能升的：{only}"
    );
    let (upgraded, _) = call_collecting(&mut client, "package.upgrade", json!({})).await;
    assert_eq!(
        upgraded["result"]["upgraded"],
        json!([{"package": "app", "from": "1.0", "to": "2.0"}]),
        "{upgraded}"
    );
    let info = client
        .call("i", "package.info", json!({"package": "app"}))
        .await;
    assert_eq!(info["result"]["version"], "2.0", "{info}");
    let local = client
        .call("i", "package.info", json!({"package": "local"}))
        .await;
    assert_eq!(local["result"]["version"], "1.0", "从文件夹装的不碰");
    // 从哪来（施工 F-12）：软件源装的是官方的，文件夹装的是本地的，出厂的是出厂；出厂的带作者、上游。
    assert_eq!(
        (&info["result"]["from"], &local["result"]["from"]),
        (&json!("official"), &json!("local"))
    );
    let listed = client.call("l", "package.list", json!({})).await;
    let item = |id: &str| {
        listed["result"]["packages"]
            .as_array()
            .and_then(|all| all.iter().find(|one| one["package"] == id).cloned())
            .unwrap_or_else(|| panic!("有 {id}：{listed}"))
    };
    assert_eq!(item("app")["from"], "official");
    assert_eq!(item("local")["from"], "local");
    let shipped = item("basesystem");
    assert_eq!(
        (&shipped["from"], &shipped["author"], &shipped["upstream"]),
        (
            &json!("shipped"),
            &json!("miyu-agent"),
            &json!("https://github.com/miyu-agent/miyu")
        ),
        "{shipped}"
    );
    let info = client
        .call("b", "package.info", json!({"package": "basesystem"}))
        .await;
    assert_eq!(
        (
            &info["result"]["author"],
            &info["result"]["upstream"],
            &info["result"]["from"]
        ),
        (
            &json!("miyu-agent"),
            &json!("https://github.com/miyu-agent/miyu"),
            &json!("shipped")
        ),
        "{info}"
    );
    let again = client.call("u", "package.upgrades", json!({})).await;
    assert_eq!(again["result"]["upgrades"], json!([]), "{again}");
}

#[tokio::test]
async fn a_held_package_is_skipped() {
    let home = Home::new();
    let (secret, trusted) = keys();
    let app1 = packed(&home, "app", "1.0", &[]);
    let app2 = packed(&home, "app", "2.0", &[]);
    let first = signed_entries(
        "2026-10-19T03:00:00Z",
        &[file_entry("app", "1.0", &[], &app1)],
        &secret,
    );
    let second = signed_entries(
        "2026-10-20T03:00:00Z",
        &[file_entry("app", "2.0", &[], &app2)],
        &secret,
    );
    let mut replies = serving(&first);
    replies.push(bytes(&app1.1));
    replies.extend(serving(&second));
    let server = Server::start(replies).await;
    let mirror = format!("{}/m", server.base_url);
    let mut client = Client::connect(core_holding(
        &home,
        "https://unreachable.invalid/",
        &[&mirror],
        trusted,
        &["app"],
    ));
    client.hello().await;
    let (reply, _) =
        call_collecting(&mut client, "package.install", json!({"source": "app"})).await;
    assert_eq!(reply["result"]["package"], "app", "{reply}");
    client
        .call("s", "package.sync", json!({"force": true}))
        .await;
    let survey = client.call("u", "package.upgrades", json!({})).await;
    assert_eq!(survey["result"]["held"], json!(["app"]), "{survey}");
    assert_eq!(survey["result"]["upgrades"], json!([]));
    let (upgraded, _) = call_collecting(&mut client, "package.upgrade", json!({})).await;
    assert_eq!(upgraded["result"]["upgraded"], json!([]), "{upgraded}");
}
