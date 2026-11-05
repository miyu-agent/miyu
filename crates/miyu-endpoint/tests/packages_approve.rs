//! 装的时候就批能力（施工 PK-3 上，设计 `61-装包说清来源和能力.md` 第二、四节）：真核心走一遍，从本机文件夹装、装回出厂的。
//! 看一眼说从本机文件来、有「注意」；`approve` 写法不对的 `bad_params`，没声明的、别的编号的不理；带 `approve` 装的装上就批，
//! 表里没盖住的照旧没批；本机文件盖同编号的不接着用以前批的；`always` 的当场拉起、`manual` 的打开不用再批，没带的照旧要批；
//! 装回出厂的不写 `approved`；写不进开关文件的装照算成、记 `WARN`。

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::extensions::{Program, quick, status, until_state};
use crate::support::*;

/// 程序一定不在的那个名字：只装、不拉起的用。
const NOWHERE: &str = "miyu-no-such-program-anywhere";

/// 工作目录里放一个扩展包 `id`：程序 `program`（测试扩展说一声好、接着服务），`start` 照写，要 `capabilities` 这几样。
fn folder(home: &Home, id: &str, (program, start): (&str, &str), capabilities: &[&str]) -> PathBuf {
    let dir = home.work.join(id);
    std::fs::create_dir_all(&dir).expect("建得了");
    let names: Vec<String> = capabilities.iter().map(|one| format!("{one:?}")).collect();
    std::fs::write(
        dir.join("package.toml"),
        format!(
            "[package]\nprotocol = [1, 1]\nname = {{ en = \"Kit\" }}\n\n[command]\nname = \"{id}\"\nprogram = \"{program}\"\nabout = {{ en = \"K\" }}\n\n[process]\nargs = [\"hello\", \"serve\"]\nstart = \"{start}\"\ncapabilities = [{}]\n",
            names.join(", ")
        ),
    )
    .expect("写得进");
    dir
}

/// 开关文件里包 `id` 批过的；没记的是 `null`。
fn approved(home: &Home, id: &str) -> Value {
    let text = std::fs::read_to_string(home.root.extensions_file()).unwrap_or_default();
    let switches: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    switches["approved"][id].clone()
}

/// 装 `path`，带上 `approve`（`null` 的不带）。
async fn install(client: &mut Client, path: &PathBuf, approve: Value) -> Value {
    let mut params = json!({"path": path});
    if !approve.is_null() {
        params["approve"] = approve;
    }
    client.call("i", "package.install", params).await
}

fn ids(listed: &Value) -> Vec<&str> {
    listed
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|one| one["id"].as_str())
        .collect()
}

#[tokio::test]
async fn a_folder_says_it_is_a_local_file_and_only_what_was_approved_is_kept() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let kit = folder(
        &home,
        "kit",
        (NOWHERE, "manual"),
        &["network", "tools", "search"],
    );

    let preview = client
        .call(
            "p",
            "package.install",
            json!({"path": kit, "preview": true}),
        )
        .await;
    let preview = &preview["result"];
    assert_eq!(preview["source"], "file", "{preview}");
    let listed = preview["capabilities"].as_array().expect("有能力那一格");
    assert_eq!(listed.len(), 3, "{preview}");
    assert!(
        listed.iter().all(|one| one.get("held").is_none()),
        "本机文件装的没有 held"
    );
    assert_eq!(
        preview["notice"],
        json!([
            "这个包没经过官方审核。",
            "它带的程序会以你的身份在这台机器上跑，不在沙盒里。"
        ])
    );

    for wrong in [
        json!({"kit": ["teleport"]}),
        json!({"Kit Pack": ["network"]}),
    ] {
        let reply = install(&mut client, &kit, wrong).await;
        assert_eq!(reply["error"]["data"]["reason"], "bad_params", "{reply}");
    }
    assert!(
        !home.root.packages().join("kit").exists(),
        "写法不对的什么都不装"
    );

    // 声明了才记：`fs.read` 没声明不理，`other` 不是这一次装的不理；`search` 表里没写，照旧没批。
    let reply = install(
        &mut client,
        &kit,
        json!({"kit": ["network", "tools", "fs.read"], "other": ["network"]}),
    )
    .await;
    assert_eq!(reply["result"]["package"], "kit", "{reply}");
    assert_eq!(approved(&home, "kit"), json!(["network", "tools"]));
    assert_eq!(approved(&home, "other"), Value::Null);
    assert_eq!(
        status(&mut client, "kit").await["unapproved"],
        json!(["search"])
    );

    // 本机文件盖同编号的：前面批过的不接着用，只记这一次批的；什么都不批的整项去掉。
    let reply = install(&mut client, &kit, json!({"kit": ["search"]})).await;
    assert_eq!(reply["result"]["package"], "kit", "{reply}");
    assert_eq!(approved(&home, "kit"), json!(["search"]));
    let reply = install(&mut client, &kit, Value::Null).await;
    assert_eq!(reply["result"]["package"], "kit", "{reply}");
    assert_eq!(approved(&home, "kit"), Value::Null);
    let left = status(&mut client, "kit").await;
    assert_eq!(
        left["unapproved"].as_array().map(Vec::len),
        Some(3),
        "{left}"
    );
    assert_eq!(ids(&left["capabilities"]).len(), 3);
}

#[tokio::test]
async fn an_approved_extension_runs_at_once_or_opens_without_asking() {
    let home = Home::new();
    let program = Program::new();
    let core = Arc::new(
        home.core_full(&Script::new([]), miyu_tool::Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;

    let always = folder(&home, "kalways", (&program.name(), "always"), &["network"]);
    let reply = install(&mut client, &always, json!({"kalways": ["network"]})).await;
    assert_eq!(reply["result"]["package"], "kalways", "{reply}");
    until_state(&mut client, "kalways", |one| one["state"] == "running").await;

    let manual = folder(&home, "kmanual", (&program.name(), "manual"), &["network"]);
    install(&mut client, &manual, json!({"kmanual": ["network"]})).await;
    let opened = client
        .call("e", "extension.enable", json!({"package": "kmanual"}))
        .await;
    assert_eq!(
        opened["result"]["on"], true,
        "装的时候批过的打开不用再批：{opened}"
    );

    let asking = folder(&home, "kasking", (&program.name(), "manual"), &["network"]);
    install(&mut client, &asking, Value::Null).await;
    let refused = client
        .call("e", "extension.enable", json!({"package": "kasking"}))
        .await;
    assert_eq!(
        refused["error"]["data"]["reason"], "needs_approval",
        "{refused}"
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn a_shipped_package_brought_back_records_no_approval() {
    let home = Home::new();
    let resources = shipped::resources(&home);
    let core = shipped::core(&home, &resources, &Script::new([]));
    let mut client = Client::connect(Arc::new(core));
    client.hello().await;
    let removed = client
        .call("r", "package.remove", json!({"package": shipped::ID}))
        .await;
    assert!(removed.get("result").is_some(), "{removed}");
    let preview = client
        .call(
            "p",
            "package.install",
            json!({"package": shipped::ID, "preview": true}),
        )
        .await;
    assert_eq!(preview["result"]["source"], "shipped", "{preview}");
    assert!(
        preview["result"].get("notice").is_none(),
        "出厂的不说没审核"
    );
    let back = client
        .call(
            "i",
            "package.install",
            json!({"package": shipped::ID, "approve": {"courier": ["tools"]}}),
        )
        .await;
    assert_eq!(back["result"]["package"], shipped::ID, "{back}");
    assert_eq!(
        approved(&home, shipped::ID),
        Value::Null,
        "出厂的算全批过、不记"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn an_approval_that_cannot_be_written_still_installs_and_warns() {
    let memory = miyu_log::Memory::new();
    let _logging = tracing::subscriber::set_default(miyu_log::subscriber(
        memory.clone(),
        miyu_log::LevelFilter::INFO,
        None,
    ));
    let home = Home::new();
    // 开关文件的位置是个目录：读不成、写不进。
    let switches = home.root.extensions_file();
    std::fs::create_dir_all(switches.join("blocked")).expect("建得了");
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let kit = folder(&home, "kit", (NOWHERE, "manual"), &["network"]);
    let reply = install(&mut client, &kit, json!({"kit": ["network"]})).await;
    assert_eq!(reply["result"]["package"], "kit", "装照算成：{reply}");
    let lines = memory.lines();
    assert!(
        lines
            .iter()
            .any(|line| line.contains("WARN") && line.contains("extension approval not recorded")),
        "{lines:?}"
    );
    assert_eq!(
        status(&mut client, "kit").await["unapproved"],
        json!(["network"]),
        "没批上的照旧停在要批准"
    );
}
