//! 装包分阶段（施工 IN-3，设计 `49-装包照pacman.md` 第三节第 3、4 条）：从软件源装之前看一眼多推荐的（没装的、去重）和装好以后
//! 一共多大（有一个没写的不写）；真装推 `package.progress` 带 `stage`、`index`、`count`：先全下完，再一个一个核对，再一个一个装；
//! 从包文件夹装的不推下载，核对、安装各推一次。

use serde_json::{Value, json};

use miyu_http::testkit::Server;

use crate::support::source::{
    bytes, call_collecting, core, file_entry, keys, packed, serving, signed_entries,
};
use crate::support::*;

/// 索引里的一条：照真的包文件，另写推荐的、装好以后多大（`None` 的不写）。
fn entry(
    id: &str,
    depends: &[&str],
    recommends: &[&str],
    installed: Option<u64>,
    file: &(String, Vec<u8>),
) -> Value {
    let mut one = file_entry(id, "1.0", depends, file);
    one["recommends"] = json!(recommends);
    if let Some(bytes) = installed {
        one["installed"] = json!(bytes);
    }
    one
}

/// 推来的进度里，核对、安装两段和每个包下完的那一次：（阶段、编号、第几个、一共几个）。
fn stages(pushed: &[Value]) -> Vec<(String, String, u64, u64)> {
    pushed
        .iter()
        .filter(|push| push["method"] == "package.progress")
        .map(|push| &push["params"])
        .filter(|params| params["stage"] != "download" || params["done"] == params["total"])
        .map(|params| {
            let text = |key: &str| params[key].as_str().unwrap_or_default().to_string();
            let number = |key: &str| params[key].as_u64().unwrap_or_default();
            (
                text("stage"),
                text("package"),
                number("index"),
                number("count"),
            )
        })
        .collect()
}

fn step(stage: &str, id: &str, index: u64, count: u64) -> (String, String, u64, u64) {
    (stage.to_string(), id.to_string(), index, count)
}

#[tokio::test]
async fn the_preview_lists_recommends_and_the_install_pushes_three_stages() {
    let home = Home::new();
    let (secret, trusted) = keys();
    let app = packed(&home, "app", "1.0", &["lib"]);
    let lib = packed(&home, "lib", "1.0", &[]);
    let rec = packed(&home, "rec", "1.0", &[]);
    let rec2 = packed(&home, "rec2", "1.0", &[]);
    let have = packed(&home, "have", "1.0", &[]);
    let loose = packed(&home, "loose", "1.0", &["bare"]);
    let bare = packed(&home, "bare", "1.0", &[]);
    let index = signed_entries(
        "2026-10-19T03:00:00Z",
        &[
            // 推荐里有要一起装的、索引里没有的、装着的、写了两遍的：都不列。
            entry(
                "app",
                &["lib"],
                &["rec", "lib", "nothere", "have", "rec"],
                Some(100),
                &app,
            ),
            entry("lib", &[], &["rec2"], Some(20), &lib),
            entry("rec", &[], &[], Some(7), &rec),
            entry("rec2", &[], &[], None, &rec2),
            entry("have", &[], &[], Some(1), &have),
            entry("loose", &["bare"], &[], Some(3), &loose),
            entry("bare", &[], &[], None, &bare),
        ],
        &secret,
    );
    let mut replies = serving(&index);
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

    // 从包文件夹装 `have`：不推下载，核对、安装各一次，第 1 个、一共 1 个。
    let folder = home.work.join("src").join("have-1.0").join("have");
    let (reply, pushed) =
        call_collecting(&mut client, "package.install", json!({"path": folder})).await;
    assert_eq!(reply["result"]["package"], "have", "{reply}");
    assert_eq!(
        stages(&pushed),
        [step("verify", "have", 1, 1), step("install", "have", 1, 1)],
        "{pushed:?}"
    );

    let (preview, _) = call_collecting(
        &mut client,
        "package.install",
        json!({"source": "app", "preview": true}),
    )
    .await;
    let preview = &preview["result"];
    assert_eq!(
        preview["recommends"],
        json!([
            {"package": "rec2", "version": "1.0", "download": rec2.1.len()},
            {"package": "rec", "version": "1.0", "download": rec.1.len(), "installed_size": 7},
        ]),
        "照要装的先后（依赖在前），装着的、一起装的、找不到的、重了的不列：{preview}"
    );
    assert_eq!(preview["installed"], 120, "要装的两个加起来，推荐的不算");
    let (loose_preview, _) = call_collecting(
        &mut client,
        "package.install",
        json!({"source": "loose", "preview": true}),
    )
    .await;
    assert!(
        loose_preview["result"].get("installed").is_none(),
        "依赖的索引没写装好以后多大，这一格不写：{loose_preview}"
    );
    assert_eq!(loose_preview["result"]["recommends"], json!([]));

    let (reply, pushed) =
        call_collecting(&mut client, "package.install", json!({"source": "app"})).await;
    assert_eq!(reply["result"]["package"], "app", "{reply}");
    assert_eq!(
        stages(&pushed),
        [
            step("download", "lib", 1, 2),
            step("download", "app", 2, 2),
            step("verify", "lib", 1, 2),
            step("verify", "app", 2, 2),
            step("install", "lib", 1, 2),
            step("install", "app", 2, 2),
        ],
        "先全下完，再核对，再装：{pushed:?}"
    );
}
