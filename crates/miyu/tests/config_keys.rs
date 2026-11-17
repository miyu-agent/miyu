//! 真核心上 `miyu config` 认人敲的键（施工 8-3 补，`docs/blueprint/cli/config.md`「怎么走」第 8 条）：带多余引号的模型键
//! 照核心的写法规整，`set`、`explain`、`unset` 找得到回应；模型那一块是 `next_turn`，说「下一轮生效」。

use crate::support::cli::{run, stderr, stdout};
use crate::support::{Home, within};
use miyu_ipc::connect_or_start;

/// 施工 8-3 补：人敲带多余引号的键（模型名里没有点），`set`、`explain`、`unset` 照规整过的键找得到核心的回应、印出来；
/// 模型那一块是 `next_turn`，说「下一轮生效」，不说「当场生效」。
#[tokio::test]
async fn a_quoted_model_key_is_found_and_next_turn_is_said() {
    let home = Home::new();
    let root = home.root.path().to_path_buf();
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let cwd = std::env::temp_dir();
    let typed = "providers.dev.models.\"m-1\".window";
    let key = "providers.dev.models.m-1.window";
    let set = run(
        &root,
        &cwd,
        "zh_CN.UTF-8",
        &["config", "set", typed, "4096"],
    )
    .await;
    assert_eq!(
        (set.status.code(), stderr(&set)),
        (Some(0), format!("已设置 {key} = 4096（下一轮生效）\n"))
    );
    let explain = run(&root, &cwd, "zh_CN.UTF-8", &["config", "explain", typed]).await;
    assert_eq!(explain.status.code(), Some(0), "{explain:?}");
    let shown = stdout(&explain);
    let first = shown.lines().next().unwrap_or_default();
    assert!(
        first.contains(&format!("（{key}）")) && first.ends_with("下一轮生效。"),
        "{shown}"
    );
    assert!(
        shown
            .lines()
            .any(|line| line.trim_start().starts_with("4096") && line.contains("系统配置")),
        "{shown}"
    );
    let unset = run(&root, &cwd, "zh_CN.UTF-8", &["config", "unset", typed]).await;
    assert_eq!(unset.status.code(), Some(0), "{unset:?}");
    assert!(
        stderr(&unset).starts_with(&format!("已删除 {key}，现在是 ")),
        "{unset:?}"
    );
    let again = run(&root, &cwd, "zh_CN.UTF-8", &["config", "unset", typed]).await;
    assert_eq!(
        (again.status.code(), stderr(&again)),
        (Some(0), format!("系统配置中没有 {key}，未修改\n"))
    );
    drop(held);
}

/// 施工 CK-1：默认预设、默认人格指着现在没有的，`config set` 照收、接一行告警，`check` 报警告，照界面语言（英文）说。
#[tokio::test]
async fn a_default_that_is_not_there_is_set_with_a_warning_and_checked() {
    let home = Home::new();
    let root = home.root.path().to_path_buf();
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let cwd = std::env::temp_dir();
    let set = run(
        &root,
        &cwd,
        "en_US.UTF-8",
        &["config", "set", "preset.default", "gone"],
    )
    .await;
    assert_eq!(
        (set.status.code(), stderr(&set)),
        (
            Some(0),
            "set preset.default = \"gone\" (applies to new sessions)\nmiyu: warning: preset.default names preset gone, which doesn't exist yet; it works once the preset is created\n".to_string()
        )
    );
    let set = run(
        &root,
        &cwd,
        "en_US.UTF-8",
        &["config", "set", "persona.default", "ghost"],
    )
    .await;
    assert_eq!(set.status.code(), Some(0), "{set:?}");
    let checked = run(&root, &cwd, "en_US.UTF-8", &["check"]).await;
    let shown = stdout(&checked);
    for line in [
        "warning: persona.default names persona ghost, which doesn't exist yet; it works once the persona is created",
        "warning: preset.default names preset gone, which doesn't exist yet; it works once the preset is created",
    ] {
        assert!(shown.contains(line), "{shown}");
    }
    assert_eq!(checked.status.code(), Some(0), "只有警告：{checked:?}");
    drop(held);
}
