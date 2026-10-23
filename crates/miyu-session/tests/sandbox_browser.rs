//! 沙盒里的 chromium（施工 SB-1 修二，主人验收撞见）：`~/.config/chromium` 是私密目录，沙盒把它藏成只读的空目录；加了
//! `--no-sandbox --user-data-dir` 的 chromium 照 `XDG_CONFIG_HOME` 在那里建 crash 上报的库，建不了就崩（SIGTRAP），结果末尾
//! 接上提示；照提示在这一条命令前加 `XDG_CONFIG_HOME=<临时目录>` 就截得出图。真的经助手跑 `shell`，家目录照场地的假家目录
//! （这台机器真的家目录不一定有 `~/.config/chromium`）。没装 chromium、没有收紧手段的机器（CI）到此为止。

use std::path::{Path, PathBuf};

use miyu_kernel::event::{Body, Level, Permission, ToolStatus};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

const PAGE: &str = "data:text/html,<h1>hi</h1>";

/// 在 `work` 里经助手跑一条 `shell`，交回状态和给她看的字。
async fn shell(home: &Home, helper: PathBuf, work: &Path, command: &str) -> (ToolStatus, String) {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools = Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来"))
        .expect("合写法");
    let args = serde_json::json!({ "command": command, "description": "Test" }).to_string();
    let script = Script::new([Play::calls(&[("shell", args.as_str())]), Play::Says("好。")]);
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: false,
        cwd: work.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(helper),
        sandbox_cache: None,
    };
    let handle = home.create_as(&script, &tools, opening).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let result = home
        .log(handle.id())
        .into_iter()
        .find_map(|event| match event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .expect("有一条工具结果");
    let text = result
        .blocks
        .iter()
        .map(|block| match block {
            miyu_kernel::block::Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect();
    (result.status, text)
}

#[tokio::test]
async fn chromium_in_the_sandbox_needs_its_config_dir_moved_and_is_told_so() {
    let helper = miyu_sandbox::testkit::built_helper();
    let probe = miyu_sandbox::probe(&helper, std::time::Duration::from_secs(60)).expect("探得了");
    let installed = std::process::Command::new("chromium")
        .arg("--version")
        .output()
        .is_ok();
    if probe.mechanisms.is_empty() || !installed {
        return;
    }
    let home = Home::outside_temp();
    let work = home.scratch.0.join("work");
    std::fs::create_dir_all(home.home.join(".config/chromium/Crash Reports")).expect("建得了目录");
    let at_home = format!("export HOME='{}'; ", home.home.display());
    let flags = format!(
        "--headless --no-sandbox --user-data-dir=$(mktemp -d) --screenshot=out.png '{PAGE}'"
    );
    let (status, text) = shell(
        &home,
        helper.clone(),
        &work,
        &format!("{at_home}chromium {flags}"),
    )
    .await;
    assert_eq!(status, ToolStatus::Error, "{text}");
    assert!(
        text.trim_end().ends_with("<your other arguments>.") && text.contains("XDG_CONFIG_HOME"),
        "崩了，末尾教她：{text}"
    );
    assert!(!work.join("out.png").exists());
    let (status, text) = shell(
        &home,
        helper,
        &work,
        &format!("{at_home}XDG_CONFIG_HOME=$(mktemp -d) chromium {flags}"),
    )
    .await;
    assert_eq!(status, ToolStatus::Ok, "{text}");
    assert!(work.join("out.png").exists(), "照提示改一次就出图：{text}");
}
