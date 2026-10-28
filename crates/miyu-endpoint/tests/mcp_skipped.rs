//! MCP 服务列出来却没登记的工具（施工 X-37 第三处，`docs/construction/X-37-脚本技能的三处小毛病.md`）：软件包列表上服务那一格、
//! `mcp.list` 的一项带 `skipped`，照原名排，没有的不写；`miyu check` 一个服务报一条 `mcp_tool_skipped`；照缓存先登记的那一回
//! 不算。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::mcp::*;
use crate::support::*;

/// 测试用的服务带 `odd` 列出来、登记不上的几件：照原名排。
fn left_out() -> Value {
    json!([
        {"tool": "", "why": "name"},
        {"tool": format!("long-{}-two", "x".repeat(60)), "why": "name_taken"},
        {"tool": "loose", "why": "parameters"},
        {"tool": "stringly", "why": "parameters"},
    ])
}

#[tokio::test]
async fn tools_left_out_are_listed_and_checked() {
    let home = Home::new();
    bundle(&home, "odd", "odd", &["odd"]);
    bundle(&home, "kit", "kit", &[]);
    home.write(
        "system/config.toml",
        &format!("[mcp.probe]\ncommand = ['{SERVER}', 'odd']\ntimeout_ms = 1000\n"),
    );
    let core = configured(&home, &Script::new([]));
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    enable(&mut client, "odd").await;
    enable(&mut client, "kit").await;
    let odd = until_status(&mut client, "odd", "running").await;
    assert_eq!(odd["mcp"][0]["skipped"], left_out(), "{odd}");
    let kit = until_status(&mut client, "kit", "running").await;
    assert!(
        kit["mcp"][0].get("skipped").is_none(),
        "都登记上了的不写：{kit}"
    );
    let mine = until_listed(&mut client, "probe", |one| one["state"] == "running").await;
    assert_eq!(mine["skipped"], left_out(), "{mine}");

    let checked = client.call("k", "check", json!({})).await;
    let mut said: Vec<(String, String)> = checked["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{checked}"))
        .iter()
        .filter(|problem| problem["code"] == "mcp_tool_skipped")
        .map(|problem| {
            assert_eq!(
                (problem["kind"].clone(), problem["level"].clone()),
                (json!("mcp"), json!("warning")),
                "{problem}"
            );
            let message = problem["message"].as_str().unwrap_or_default();
            for named in [
                "(name)",
                "-two (name_taken)",
                "loose (parameters)",
                "stringly (parameters)",
            ] {
                assert!(message.contains(named), "说哪几件、为什么：{problem}");
            }
            assert!(!message.contains("-one"), "登记上了的不说：{problem}");
            let file = problem["file"].as_str().unwrap_or_default().to_string();
            (file, message.to_string())
        })
        .collect();
    said.sort();
    assert_eq!(said.len(), 2, "一个服务一条：{checked}");
    assert_eq!(
        said[0].0, "system/config.toml",
        "个人设置里的报在个人设置上：{said:?}"
    );
    assert!(said[0].1.contains("probe: "), "{said:?}");
    assert_eq!(
        said[1].0, "system/packages/odd/package.toml",
        "包带的报在清单上：{said:?}"
    );
    assert!(said[1].1.contains("odd: "), "{said:?}");
    core.stop_extensions().await;
}

#[tokio::test]
async fn a_catalog_from_the_cache_says_nothing_left_out() {
    let home = Home::new();
    let flag = home.work.join("odd-down");
    let exit_if = format!("exit-if:{}", flag.display());
    bundle(&home, "odd", "odd", &["odd", &exit_if]);
    let started = || {
        let core = Arc::new(
            home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
                .with_extension_timing(quick()),
        );
        core.start_extensions();
        core
    };
    let core = started();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    enable(&mut client, "odd").await;
    let listed = until_status(&mut client, "odd", "running").await;
    assert_eq!(listed["mcp"][0]["skipped"], left_out(), "{listed}");
    core.stop_extensions().await;
    drop(client);

    // 服务起不来了：照缓存登记了，没列过，不说没登记的（缓存里的 `stringly`、撞名的那件照缓存登记时也登记不上）。
    std::fs::write(&flag, b"").unwrap();
    let down = started();
    let mut client = Client::connect(Arc::clone(&down));
    client.hello().await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let stopped = loop {
        let one = entry(&mut client, "odd").await;
        if one["mcp"][0]["state"] == "stopped" {
            break one;
        }
        assert!(tokio::time::Instant::now() < deadline, "等不到停下：{one}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert!(stopped["mcp"][0].get("skipped").is_none(), "{stopped}");
    down.stop_extensions().await;
}
