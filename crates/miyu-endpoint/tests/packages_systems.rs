//! 包支持哪几种系统（施工 PL-2）：软件源的列表默认不列不支持本机的包，`packages.other_systems` 开着的照列、带 `unsupported`；
//! 从软件源装、从文件夹装都拒 `wrong_target`，从软件源装的照索引先拒、不下载。真核心走一遍，假服务器当软件源。

use serde_json::{Value, json};

use miyu_config::package::{System, Target};
use miyu_http::testkit::Server;

use crate::support::source::{core, entry, keys, serving, signed_entries};
use crate::support::*;

/// 不是本机的一种系统。
fn elsewhere() -> &'static str {
    System::ALL
        .into_iter()
        .find(|system| Some(*system) != System::here())
        .expect("有别的")
        .as_str()
}

/// 不是本机的一种操作系统加处理器。
fn other_target() -> &'static str {
    Target::ALL
        .into_iter()
        .find(|target| Some(*target) != Target::here())
        .expect("有别的")
        .as_str()
}

/// 三条：只支持别的系统的 `away`、没写的 `any`、只有别的系统的包文件的 `native`。
fn index(secret: &str) -> (Vec<u8>, Vec<u8>) {
    let mut away = entry("away", json!({"en": "Away"}), json!({}));
    away["systems"] = json!([elsewhere()]);
    let mut native = entry("native", json!({"en": "Native"}), json!({}));
    native["files"][0]["target"] = json!(other_target());
    signed_entries(
        "2026-11-17T03:00:00Z",
        &[away, entry("any", json!({"en": "Any"}), json!({})), native],
        secret,
    )
}

fn rows(reply: &Value) -> Vec<(String, bool)> {
    reply["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("有：{reply}"))
        .iter()
        .map(|row| {
            (
                row["package"].as_str().unwrap_or_default().to_string(),
                row["unsupported"] == true,
            )
        })
        .collect()
}

#[tokio::test]
async fn packages_for_other_systems_are_hidden_until_the_setting_is_on() {
    let home = Home::new();
    let (secret, trusted) = keys();
    let server = Server::start(serving(&index(&secret))).await;
    let mut client = Client::connect(core(&home, &server.base_url, &[], trusted));
    client.hello().await;
    let hidden = client
        .call("s1", "package.search", json!({"query": ""}))
        .await;
    assert_eq!(
        rows(&hidden),
        [("any".to_string(), false)],
        "默认不列：{hidden}"
    );
    let set = client
        .call(
            "w",
            "config.set",
            json!({"layer": "system", "changes": [{"key": "packages.other_systems", "value": true}]}),
        )
        .await;
    assert!(set.get("error").is_none(), "{set}");
    let shown = client
        .call("s2", "package.search", json!({"query": ""}))
        .await;
    assert_eq!(
        rows(&shown),
        [
            ("away".to_string(), true),
            ("any".to_string(), false),
            ("native".to_string(), true),
        ],
        "开着的照列、标上：{shown}"
    );
    // 看一眼也说不支持；装照旧拒，照索引就拒、不下载（假服务器只回了索引、签名）。
    let info = client
        .call(
            "i",
            "package.info",
            json!({"package": "away", "source": true}),
        )
        .await;
    assert_eq!(info["result"]["unsupported"], true, "{info}");
    for id in ["away", "native"] {
        let refused = client
            .call("n", "package.install", json!({"source": id}))
            .await;
        assert_eq!(reason(&refused), Some("wrong_target"), "{id}：{refused}");
        assert_eq!(refused["error"]["message"], "不支持该系统。", "{refused}");
    }
}

#[tokio::test]
async fn a_folder_for_other_systems_is_refused() {
    let home = Home::new();
    let folder = home.work.join("away");
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        format!(
            "[package]\nname = {{ en = \"Away\" }}\nsystems = [\"{}\"]\n",
            elsewhere()
        ),
    )
    .expect("写得进");
    let mut client = Client::connect(crate::support::packaged::core(&home));
    client.hello().await;
    let refused = client
        .call("i", "package.install", json!({"path": folder}))
        .await;
    assert_eq!(reason(&refused), Some("wrong_target"), "{refused}");
    // 写了本机的照装。
    let here = System::here().expect("测试跑在认得的系统上").as_str();
    std::fs::write(
        folder.join("package.toml"),
        format!(
            "[package]\nname = {{ en = \"Away\" }}\nsystems = [\"{here}\", \"{}\"]\n",
            elsewhere()
        ),
    )
    .expect("写得进");
    let installed = client
        .call("j", "package.install", json!({"path": folder}))
        .await;
    assert_eq!(installed["result"]["package"], "away", "{installed}");
}
