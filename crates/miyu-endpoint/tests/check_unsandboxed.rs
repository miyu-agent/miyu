//! `miyu check` 查管理员放行出沙盒的工具（施工 X-36，`11-权限与沙盒.md` A15）：系统配置的 `sandbox.tools.<名字>` 每一条照现在的
//! 工具目录查，目录里没有的、来处对不上的（印出现在的来处）、文件是链接的（施工 X-37）、放行了 `shell` 的各报一条警告，对得上的
//! 不报。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::tool::Access;
use miyu_session::testkit::Script;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::venues::configured_core;
use crate::support::*;

#[tokio::test]
async fn each_grant_is_checked_against_the_catalog() {
    let home = Home::new();
    home.write(
        "system/config.toml",
        "[sandbox.tools]\nlight = \"~home\"\nlamp = \"~home\"\nshell = \"basesystem\"\ngone = \"~home\"\nlinked = \"~home\"\n",
    );
    let fake = |name: &str| Fake::new(name, Access::Execute, Act::Echo) as Arc<dyn Tool>;
    let catalog = Catalog::in_packages([
        (
            "~home",
            vec![
                fake("light"),
                Fake::with_file(
                    "linked",
                    Access::Execute,
                    home.work.join("elsewhere.py"),
                    Act::Echo,
                ) as Arc<dyn Tool>,
            ],
        ),
        ("smart-home", vec![fake("lamp")]),
        ("basesystem", vec![fake("shell")]),
    ])
    .expect("合写法");
    let mut client = Client::connect(configured_core(&home, &Script::new([]), catalog));
    client.hello().await;
    let reply = client.call("k", "check", json!({})).await;
    let problems: Vec<Value> = reply["result"]["problems"]
        .as_array()
        .cloned()
        .unwrap_or_else(|| panic!("{reply}"))
        .into_iter()
        .filter(|problem| {
            problem["code"]
                .as_str()
                .is_some_and(|code| code.starts_with("sandbox_"))
        })
        .collect();
    let codes: Vec<&str> = problems
        .iter()
        .map(|problem| problem["code"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        codes,
        [
            "sandbox_tool_missing",
            "sandbox_tool_moved",
            "sandbox_tool_linked",
            "sandbox_shell"
        ],
        "照工具名排：gone、lamp、linked、shell；light 对得上不报：{problems:?}"
    );
    assert!(problems.iter().all(|problem| problem["level"] == "warning"
        && problem["kind"] == "config"
        && problem["file"] == "system/config.toml"));
    let linked = problems[2]["message"].as_str().unwrap_or_default();
    assert!(linked.contains("elsewhere.py"), "印出它的位置：{linked}");
    let moved = problems[1]["message"].as_str().unwrap_or_default();
    assert!(
        moved.contains("~home") && moved.contains("smart-home"),
        "写的和现在的来处都印出来：{moved}"
    );
}

/// 整个 `scripts/` 是指到工作区的链接（施工 X-37 补）：里面的脚本文件本身不是链接，放行照样不算，报 `sandbox_tool_linked`、
/// 印出真实位置（在工作区里）。Unix 上（Windows 上建链接要权限，判法的单元测试在 `miyu-session` 里）。
#[cfg(unix)]
#[tokio::test]
async fn a_scripts_folder_that_links_out_is_not_granted() {
    let home = Home::new();
    home.write("system/config.toml", "[sandbox.tools]\nrun = \"~home\"\n");
    let real = home.work.join("scripts");
    std::fs::create_dir_all(&real).expect("建得了");
    std::fs::write(real.join("run.py"), "x").expect("写得进");
    let own = home.root.path().join("home/alice");
    std::fs::create_dir_all(&own).expect("建得了");
    std::os::unix::fs::symlink(&real, own.join("scripts")).expect("建得了链接");
    let tool = Fake::with_file(
        "run",
        Access::Execute,
        own.join("scripts/run.py"),
        Act::Echo,
    ) as Arc<dyn Tool>;
    let catalog = Catalog::in_packages([("~home", vec![tool])]).expect("合写法");
    let mut client = Client::connect(configured_core(&home, &Script::new([]), catalog));
    client.hello().await;
    let reply = client.call("k", "check", json!({})).await;
    let linked: Vec<&Value> = reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|problem| problem["code"] == "sandbox_tool_linked")
        .collect();
    assert_eq!(linked.len(), 1, "{reply}");
    let message = linked[0]["message"].as_str().unwrap_or_default();
    let shown = std::fs::canonicalize(real.join("run.py")).expect("在");
    assert!(
        message.contains(&shown.display().to_string()),
        "印出真实位置：{message}"
    );
}
