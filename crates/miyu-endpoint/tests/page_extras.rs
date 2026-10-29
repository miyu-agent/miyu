//! 技能、脚本、MCP 三页另要的两样（施工 X-27 补，网页报的）：`skill.list`、`script.list` 顶上带 `dir`，你自己放的那个
//! 文件夹，一个都没有、还没建的也照写；软件包列表上包带的 MCP 服务每一格带 `on`，同 `mcp.list` 的一项，改了全局开关跟着变。
//! 加载方式那一格施工 FL-1 拆了：工具一律全量，`tool.set_loading` 照旧回 `{}`、什么都不改。

use std::sync::Arc;

use serde_json::json;

use miyu_session::testkit::Script;

use crate::support::extensions::*;
use crate::support::mcp::*;
use crate::support::*;

#[tokio::test]
async fn both_lists_name_the_folder_even_when_nothing_is_in_it() {
    let home = Home::new();
    let mut client = Client::connect(crate::scripts::core(&home, &Script::new([])));
    client.hello().await;
    for (method, folder) in [("skill.list", "skills"), ("script.list", "scripts")] {
        let reply = client.call("l", method, json!({})).await;
        let dir = reply["result"]["dir"]
            .as_str()
            .unwrap_or_else(|| panic!("{reply}"))
            .replace('\\', "/");
        let root = home
            .root
            .path()
            .file_name()
            .expect("有名字")
            .to_string_lossy();
        assert!(dir.ends_with(&format!("{root}/{folder}")), "{dir}");
    }
}

#[tokio::test]
async fn a_bundled_service_shows_its_switch_and_follows_changes() {
    let home = Home::new();
    bundle(&home, "probe", "probe", &[]);
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let before = entry(&mut client, "probe").await["mcp"][0].clone();
    assert_eq!(before["on"], json!(true), "{before}");
    assert!(before.get("need_load").is_none(), "{before}");
    let set = client
        .call(
            "t",
            "tool.set_loading",
            json!({"server": "probe", "need_load": true}),
        )
        .await;
    assert_eq!(set["result"], json!({}), "{set}");
    let off = client
        .call(
            "f",
            "feature.set",
            json!({"feature": "mcp-probe", "on": false}),
        )
        .await;
    assert_eq!(off["result"], json!({}), "{off}");
    let after = entry(&mut client, "probe").await["mcp"][0].clone();
    assert_eq!(after["on"], json!(false), "{after}");
    assert!(after.get("need_load").is_none(), "{after}");
    assert!(!settings(&home).contains("loading"), "{}", settings(&home));
    core.stop_extensions().await;
}

/// 配置的字。
fn settings(home: &Home) -> String {
    std::fs::read_to_string(home.root.path().join("config.toml")).unwrap_or_default()
}

#[tokio::test]
async fn a_written_server_comes_back_whole_and_saving_it_again_changes_nothing() {
    let home = Home::new();
    home.root.prepare_workspace().expect("建得了家目录");
    let core = configured(&home, &Script::new([]));
    let _watching = core.watch_config().expect("监视得了");
    let _following = core.follow_home_mcp();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let env = json!({
        "MIYU_MCP_PROBE": "mine",
        "TOKEN": {"secret": "github"},
        "PATH_FROM": {"env": "HOME"},
    });
    let set = client
        .call(
            "s1",
            "mcp.set",
            json!({
                "name": "probe",
                "command": SERVER,
                "args": ["legacy"],
                "env": env,
                "venues": ["local"],
                "timeout_ms": 1000,
            }),
        )
        .await;
    assert_eq!(set["result"], json!({"name": "probe"}), "{set}");
    let first = settings(&home);
    let listed = until_listed(&mut client, "probe", |one| one["env"].is_object()).await;
    assert_eq!(
        (&listed["env"], &listed["timeout_ms"], &listed["command"]),
        (&env, &json!(1000), &json!([SERVER, "legacy"])),
        "{listed}"
    );

    // 照列出来的再存一次：`command` 拆成程序和参数，别的照原样交回。
    let command = listed["command"].as_array().expect("是数组");
    let again = client
        .call(
            "s2",
            "mcp.set",
            json!({
                "name": "probe",
                "command": command[0],
                "args": command[1..],
                "env": listed["env"],
                "venues": listed["venues"],
                "timeout_ms": listed["timeout_ms"],
            }),
        )
        .await;
    assert_eq!(again["result"], json!({"name": "probe"}), "{again}");
    assert_eq!(settings(&home), first, "存回去一个字不变");

    // 没写的时限、环境变量不带。
    let plain = client
        .call("s3", "mcp.set", json!({"name": "probe", "command": SERVER}))
        .await;
    assert_eq!(plain["result"], json!({"name": "probe"}), "{plain}");
    let bare = until_listed(&mut client, "probe", |one| one.get("env").is_none()).await;
    assert!(bare.get("timeout_ms").is_none(), "{bare}");
}
