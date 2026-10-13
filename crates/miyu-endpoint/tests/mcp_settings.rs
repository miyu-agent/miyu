//! 个人设置里的 MCP 服务（施工 F-9，`docs/blueprint/mcp.md`「个人设置里的一张表」）：`miyu check` 照行报写错的、报盖住了包里
//! 同名服务的；`[mcp.<名字>]` 不算配置里不认识的键。

use serde_json::{Value, json};

use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::venues::configured_core;
use crate::support::*;

/// 带一个叫 `github` 的服务的包。
const KIT: &str = "[package]\nname = { en = \"Kit\" }\n\n[[mcp]]\nname = \"github\"\ncommand = [\"kit-github\"]\n";

/// 个人设置：界面语言，一个盖住包里那个的服务，一个写错的。
const SETTINGS: &str = "[ui]\nlanguage = \"zh\"\n\n[mcp.github]\ncommand = [\"npx\", \"server-github\"]\n\n[mcp.empty]\ncommand = []\n";

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(configured_core(home, &Script::new([]), Catalog::default()));
    client.hello().await;
    client
}

/// 一处的几格：种类、文件、代码、级别、第几行。
fn brief(problem: &Value) -> (String, String, String, String, Option<u64>) {
    let text = |key: &str| problem[key].as_str().unwrap_or_default().to_string();
    (
        text("kind"),
        text("file"),
        text("code"),
        text("level"),
        problem["line"].as_u64(),
    )
}

fn row(
    file: &str,
    code: &str,
    level: &str,
    line: u64,
) -> (String, String, String, String, Option<u64>) {
    (
        "mcp".to_string(),
        file.to_string(),
        code.to_string(),
        level.to_string(),
        Some(line),
    )
}

#[tokio::test]
async fn check_reports_wrong_servers_and_covered_package_servers() {
    let home = Home::new();
    home.write("home/alice/packages/kit/package.toml", KIT);
    home.write("home/alice/settings.toml", SETTINGS);
    let mut client = connected(&home).await;
    let reply = client.call("k1", "check", json!({})).await;
    let problems = reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .clone();
    let found: Vec<_> = problems.iter().map(brief).collect();
    assert_eq!(
        found,
        [
            row("home/alice/settings.toml", "bad_mcp", "error", 7),
            row(
                "home/alice/packages/kit/package.toml",
                "mcp_covered",
                "warning",
                4
            ),
        ],
        "[mcp.<名字>] 不算不认识的键：{reply}"
    );
    let covered = problems[1]["message"].as_str().unwrap_or_default();
    assert!(
        covered.contains("home/alice/settings.toml [mcp.github]"),
        "说被谁盖住：{covered}"
    );
    // 只查个人设置这一份：照样报，盖住了的也报。
    let path = home.root.path().join("home/alice/settings.toml");
    let one = client
        .call("k2", "check", json!({"file": path.to_string_lossy()}))
        .await;
    let found: Vec<_> = one["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{one}"))
        .iter()
        .map(brief)
        .collect();
    assert_eq!(found.len(), 2, "{one}");
}

#[tokio::test]
async fn the_mcp_table_does_not_disturb_the_rest_of_the_settings() {
    let home = Home::new();
    home.write("home/alice/settings.toml", SETTINGS);
    let mut client = connected(&home).await;
    let reply = client
        .call("k1", "config.get", json!({"keys": ["ui.language"]}))
        .await;
    assert_eq!(
        reply["result"]["items"]["ui.language"]["value"], "zh",
        "{reply}"
    );
    assert_eq!(reply["result"]["problems"], json!([]), "{reply}");
}
