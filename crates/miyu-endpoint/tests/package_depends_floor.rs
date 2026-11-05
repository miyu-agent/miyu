//! 依赖的版本下限（施工 PK-2，`docs/blueprint/packages.md`「依赖」）：真核心走一遍。`[depends] packages` 写 `<编号>>=<版本>`，
//! 装着的版本照 semver 比；不够的装不上（`data.missing` 照写的原样），装着的不够了列表、`miyu check` 同缺了；从软件源装的照
//! 下限挑、装着的不够一起升；不写版本的照旧。

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use miyu_http::testkit::Server;
use miyu_session::testkit::Script;

use crate::support::source::{
    bytes, call_collecting, core, file_entry, keys, packed, serving, signed_entries,
};
use crate::support::*;

/// 在工作目录里放一个要装的包（`<编号>-<版本>/` 底下的 `<编号>/`），版本没有的不写，`links` 接在后面；交回文件夹的路径。
fn source(home: &Home, id: &str, version: Option<&str>, links: &str) -> PathBuf {
    let folder = home
        .work
        .join(format!("{id}-{}", version.unwrap_or("none")))
        .join(id);
    std::fs::create_dir_all(&folder).expect("建得了");
    let version = version.map_or(String::new(), |v| format!("version = \"{v}\"\n"));
    let text = format!("[package]\n{version}name = {{ en = \"{id}\" }}\n{links}");
    std::fs::write(folder.join("package.toml"), text).expect("写得进");
    folder
}

async fn install(client: &mut Client, folder: &Path, preview: bool) -> Value {
    client
        .call(
            "i",
            "package.install",
            json!({"path": folder.to_string_lossy(), "preview": preview}),
        )
        .await
}

async fn listed(client: &mut Client, id: &str) -> Value {
    let reply = client.call("l", "package.list", json!({})).await;
    reply["result"]["packages"]
        .as_array()
        .expect("有")
        .iter()
        .find(|one| one["package"] == id)
        .cloned()
        .unwrap_or(Value::Null)
}

/// `miyu check` 报的缺依赖的那几条的话。
async fn checked(client: &mut Client) -> Vec<String> {
    let reply = client.call("k", "check", json!({})).await;
    reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|problem| problem["code"] == "depends_missing")
        .map(|problem| problem["message"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[tokio::test]
async fn a_floor_refuses_too_old_and_an_installed_one_that_falls_short_says_so() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let addon = source(
        &home,
        "addon",
        Some("0.1.0"),
        "\n[depends]\npackages = [\"base>=1.2.0\", \"plain\"]\n",
    );
    for (version, reason) in [
        (None, "没写版本的不够"),
        (Some("1.1.9"), "比下限低"),
        (Some("1.2.0-rc.1"), "预发布的比正式的低"),
    ] {
        let base = source(&home, "base", version, "");
        let put = install(&mut client, &base, false).await;
        assert!(put.get("error").is_none(), "{put}");
        let plain = install(&mut client, &source(&home, "plain", None, ""), false).await;
        assert!(plain.get("error").is_none(), "{plain}");
        for preview in [true, false] {
            let refused = install(&mut client, &addon, preview).await;
            assert_eq!(
                refused["error"]["data"]["reason"], "depends_missing",
                "{reason}：{refused}"
            );
            assert_eq!(
                refused["error"]["data"]["missing"],
                json!(["base>=1.2.0"]),
                "{reason}：照写的原样带要的版本"
            );
        }
    }
    assert_eq!(
        listed(&mut client, "addon").await,
        Value::Null,
        "什么都没装"
    );

    let base = install(
        &mut client,
        &source(&home, "base", Some("1.10.0"), ""),
        false,
    )
    .await;
    assert!(base.get("error").is_none(), "{base}");
    let installed = install(&mut client, &addon, false).await;
    assert!(
        installed.get("error").is_none(),
        "1.10.0 照 semver 比 1.2.0 高：{installed}"
    );
    let listed_addon = listed(&mut client, "addon").await;
    assert_eq!(
        listed_addon["depends"],
        json!({"packages": ["base>=1.2.0", "plain"]}),
        "照写的原样"
    );
    assert_ne!(listed_addon["status"], "depends_missing", "{listed_addon}");
    assert!(checked(&mut client).await.is_empty());

    // 降级照装（版本只在依赖上比），依赖它的那一个同缺了依赖。
    let older = install(
        &mut client,
        &source(&home, "base", Some("1.0.0"), ""),
        false,
    )
    .await;
    assert!(older.get("error").is_none(), "{older}");
    let listed_addon = listed(&mut client, "addon").await;
    assert_eq!(listed_addon["status"], "depends_missing", "{listed_addon}");
    assert_eq!(listed_addon["missing"], json!(["base>=1.2.0"]));
    assert_eq!(
        checked(&mut client).await,
        ["缺少依赖的软件包：base>=1.2.0"]
    );
}

/// 从软件源装：依赖装着的版本不够，照下限挑索引里的一起升，先升依赖。推荐的装着、版本够的不列，不够的照列；索引里的也不够的
/// 不列（看一眼）。
#[tokio::test]
async fn installing_from_a_source_upgrades_a_dependency_below_the_floor() {
    let home = Home::new();
    for (id, version) in [("lib", "1.0.0"), ("extra", "1.0.0"), ("fine", "3.0.0")] {
        home.write(
            &format!("opt/{id}/package.toml"),
            &format!("[package]\nversion = \"{version}\"\nname = {{ en = \"{id}\" }}\n"),
        );
    }
    let (secret, trusted) = keys();
    let app = packed(&home, "app", "1.0.0", &["lib>=1.2.0"]);
    let lib = packed(&home, "lib", "1.3.0", &[]);
    let mut app_entry = file_entry("app", "1.0.0", &["lib>=1.2.0"], &app);
    app_entry["recommends"] = json!(["extra>=2.0.0", "fine>=2.0.0", "short>=5.0.0"]);
    let pair = signed_entries(
        "2026-10-19T03:00:00Z",
        &[
            app_entry,
            file_entry("extra", "2.1.0", &[], &lib),
            file_entry("fine", "3.1.0", &[], &lib),
            file_entry("short", "1.0.0", &[], &lib),
            file_entry("lib", "1.3.0", &[], &lib),
        ],
        &secret,
    );
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
    assert_eq!(
        preview["result"]["with"],
        json!([{"package": "lib", "version": "1.3.0"}]),
        "{preview}"
    );
    let recommended: Vec<(&str, &str)> = preview["result"]["recommends"]
        .as_array()
        .expect("有")
        .iter()
        .map(|one| {
            let field = |key: &str| one[key].as_str().unwrap_or_default();
            (field("package"), field("version"))
        })
        .collect();
    assert_eq!(
        recommended,
        [("extra", "2.1.0")],
        "装着够的、索引里不够的不列"
    );
    let (reply, _) =
        call_collecting(&mut client, "package.install", json!({"source": "app"})).await;
    assert_eq!(reply["result"]["with"], json!(["lib"]), "{reply}");
    assert_eq!(
        listed(&mut client, "lib").await["version"],
        "1.3.0",
        "一起升了"
    );
    let app = listed(&mut client, "app").await;
    assert_eq!(app["depends"], json!({"packages": ["lib>=1.2.0"]}));
    assert_ne!(app["status"], "depends_missing", "{app}");
}
