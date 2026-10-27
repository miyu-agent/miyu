//! 装包时查冲突（施工 F-8 六补，设计 `31-软件包.md` 第四节、定了的 I，`docs/blueprint/packages.md`「冲突」）：要装的包带的
//! 子命令、功能、工具、技能、MCP 服务、知识库，和装着的别的包重了名，什么都不动、报 `package_conflict`，`data.conflicts`
//! 一处一条（哪一样、什么名字、跟哪个包）。看一眼、装回出厂的也查；升级同一个包不算，跟家目录里自己放的同名不算（自己的赢）。

use std::path::PathBuf;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 一份只带技能、脚本、知识库的清单（`bundle`）。
const BUNDLE: &str = "[package]\nname = { en = \"Kit\" }\n";

/// 一份 `SKILL.md`。
fn skill(name: &str) -> String {
    format!("---\nname: {name}\ndescription: Do {name}.\n---\nbody\n")
}

/// 一个脚本：开头注释照旧版的写法。
fn script(name: &str) -> String {
    format!(
        "#!/bin/sh\n# Description: Echo {name}.\n# Parameters: {{\"type\": \"object\", \"properties\": {{}}}}\necho {name}\n"
    )
}

/// 在工作目录里放一个要装的包：`id` 这个文件夹，清单 `manifest`，外加 `files`（相对包文件夹的路径和内容）。
fn source(home: &Home, id: &str, manifest: &str, files: &[(&str, String)]) -> PathBuf {
    let folder = home.work.join("sources").join(id);
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(folder.join("package.toml"), manifest).expect("写得进");
    for (path, text) in files {
        let file = folder.join(path);
        std::fs::create_dir_all(file.parent().expect("有上一层")).expect("建得了");
        std::fs::write(file, text).expect("写得进");
    }
    folder
}

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    client
}

async fn install(client: &mut Client, path: &PathBuf) -> Value {
    client
        .call("i", "package.install", json!({ "path": path }))
        .await
}

async fn preview(client: &mut Client, path: &PathBuf) -> Value {
    client
        .call(
            "p",
            "package.install",
            json!({ "path": path, "preview": true }),
        )
        .await
}

/// 装成了的。
fn installed(reply: &Value) {
    assert!(reply["result"].is_object(), "装上了：{reply}");
}

/// 拒绝里列的冲突。
fn conflicts(reply: &Value) -> Vec<Value> {
    assert_eq!(reason(reply), Some("package_conflict"), "{reply}");
    reply["error"]["data"]["conflicts"]
        .as_array()
        .expect("列了冲突")
        .clone()
}

/// 列表里的这个包：写错了、撞了的带 `code`。
async fn listed(client: &mut Client, id: &str) -> Option<Value> {
    let reply = client.call("l", "package.list", json!({})).await;
    reply["result"]["packages"]
        .as_array()
        .expect("有")
        .iter()
        .find(|one| one["package"] == id)
        .cloned()
}

/// 管理员家目录那一层里的这个包。
fn placed(home: &Home, id: &str) -> bool {
    home.root.path().join("system/packages").join(id).exists()
}

#[tokio::test]
async fn a_package_taking_an_installed_name_is_refused_before_anything_moves() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let first = source(
        &home,
        "zkit",
        BUNDLE,
        &[("skills/pdf/SKILL.md", skill("pdf"))],
    );
    installed(&install(&mut client, &first).await);
    let second = source(
        &home,
        "akit",
        BUNDLE,
        &[("skills/pdf/SKILL.md", skill("pdf"))],
    );
    let wanted = vec![json!({"kind": "skill", "name": "pdf", "package": "zkit"})];
    assert_eq!(
        conflicts(&preview(&mut client, &second).await),
        wanted,
        "看一眼也拒"
    );
    assert_eq!(conflicts(&install(&mut client, &second).await), wanted);
    assert!(!placed(&home, "akit"), "什么都不动");
    let kept = listed(&mut client, "zkit").await.expect("还在");
    assert!(kept["code"].is_null(), "装着的那个照旧好好的：{kept}");
}

#[tokio::test]
async fn an_earlier_id_cannot_push_an_installed_package_out() {
    // 以前：读清单照编号先后定谁先到，编号排在前的新包装上，被判撞了的是装着的那个，悄悄失效。
    let home = Home::new();
    let mut client = connected(&home).await;
    let ui = |name: &str| {
        format!(
            "[package]\nprotocol = [1, 1]\nname = {{ en = \"{name}\" }}\n\n[command]\nname = \"xcmd\"\nprogram = \"miyu-{name}\"\nabout = {{ en = \"X\" }}\n\n[ui]\nopens = []\n"
        )
    };
    let first = source(&home, "zui", &ui("zui"), &[]);
    installed(&install(&mut client, &first).await);
    let second = source(&home, "aui", &ui("aui"), &[]);
    assert_eq!(
        conflicts(&install(&mut client, &second).await),
        vec![json!({"kind": "command", "name": "xcmd", "package": "zui"})]
    );
    assert!(!placed(&home, "aui"));
    let kept = listed(&mut client, "zui").await.expect("还在");
    assert!(kept["code"].is_null(), "装着的那个没被挤掉：{kept}");
}

#[tokio::test]
async fn scripts_mcp_services_and_knowledge_bases_count_too() {
    let home = Home::new();
    let mut client = connected(&home).await;
    // 脚本的工具名撞了出厂的工具（基础系统的 read）。
    let reader = source(
        &home,
        "xreader",
        BUNDLE,
        &[("scripts/read.sh", script("read"))],
    );
    assert_eq!(
        conflicts(&install(&mut client, &reader).await),
        vec![json!({"kind": "tool", "name": "read", "package": "read"})]
    );
    let mcp =
        "[package]\nname = { en = \"M\" }\n\n[[mcp]]\nname = \"lone\"\ncommand = [\"server\"]\n";
    let first = source(&home, "zmcp", mcp, &[]);
    installed(&install(&mut client, &first).await);
    let second = source(&home, "amcp", mcp, &[]);
    assert_eq!(
        conflicts(&install(&mut client, &second).await),
        vec![json!({"kind": "mcp", "name": "lone", "package": "zmcp"})]
    );
    let first = source(&home, "zkb", BUNDLE, &[("kb/notes/a.md", "a".into())]);
    installed(&install(&mut client, &first).await);
    let second = source(
        &home,
        "akb",
        BUNDLE,
        &[
            ("kb/notes/b.md", "b".into()),
            ("skills/notes/SKILL.md", skill("notes")),
        ],
    );
    assert_eq!(
        conflicts(&install(&mut client, &second).await),
        vec![json!({"kind": "knowledge", "name": "notes", "package": "zkb"})],
        "技能 notes 和知识库 notes 不是一样东西，不算撞"
    );
}

#[tokio::test]
async fn upgrading_itself_or_shadowing_ones_own_is_not_a_conflict() {
    let home = Home::new();
    home.write("home/alice/skills/notes/SKILL.md", &skill("notes"));
    let mut client = connected(&home).await;
    let kit = source(
        &home,
        "kit",
        BUNDLE,
        &[
            ("skills/notes/SKILL.md", skill("notes")),
            ("skills/pdf/SKILL.md", skill("pdf")),
        ],
    );
    installed(&install(&mut client, &kit).await);
    installed(&install(&mut client, &kit).await);
}

#[tokio::test]
async fn bringing_a_shipped_package_back_checks_too() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let removed = client
        .call("r", "package.remove", json!({"package": "authoring"}))
        .await;
    assert!(removed["result"].is_object(), "{removed}");
    let mine = source(
        &home,
        "mykit",
        BUNDLE,
        &[("skills/skill-creator/SKILL.md", skill("skill-creator"))],
    );
    installed(&install(&mut client, &mine).await);
    let back = client
        .call("b", "package.install", json!({"package": "authoring"}))
        .await;
    assert_eq!(
        conflicts(&back),
        vec![json!({"kind": "skill", "name": "skill-creator", "package": "mykit"})]
    );
    let peek = client
        .call(
            "bp",
            "package.install",
            json!({"package": "authoring", "preview": true}),
        )
        .await;
    assert_eq!(reason(&peek), Some("package_conflict"), "{peek}");
}

/// 斜杠命令模板（施工 X-14 上补）：和装着的包的撞了不装，和核心内置的名字、别名撞了也不装（记在拥有它的包上）；看一眼列出
/// 包带的模板 `slash`。
#[tokio::test]
async fn slash_command_templates_count_and_the_builtins_too() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let first = source(
        &home,
        "zkit",
        BUNDLE,
        &[("commands/review.md", "Review.\n".into())],
    );
    installed(&install(&mut client, &first).await);
    let second = source(
        &home,
        "akit",
        BUNDLE,
        &[
            ("commands/review.md", "Mine.\n".into()),
            ("commands/remember.md", "Mine.\n".into()),
            ("commands/reset.md", "Mine.\n".into()),
        ],
    );
    assert_eq!(
        conflicts(&install(&mut client, &second).await),
        vec![
            json!({"kind": "slash", "name": "remember", "package": "memory"}),
            // 核心自带的（施工 BS-1 下下）写 `core: true`。
            json!({"kind": "slash", "name": "reset", "core": true}),
            json!({"kind": "slash", "name": "review", "package": "zkit"}),
        ]
    );
    let third = source(
        &home,
        "bkit",
        BUNDLE,
        &[("commands/fresh.md", "Fresh.\n".into())],
    );
    let peek = preview(&mut client, &third).await;
    assert_eq!(peek["result"]["slash"], json!(["fresh"]), "{peek}");
}
