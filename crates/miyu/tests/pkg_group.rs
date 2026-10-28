//! 真跑 `miyu pkg` 装回、卸掉整个组（施工 BS-1 下中，`docs/blueprint/cli/pkg.md`，照 pacman 写组名）：卸 `basesystem` 先印要卸
//! 哪几个、问一句，答了一个一行说卸掉了谁；装回来同样；组里没有要动的说一句。

use crate::support::cli::{run, stdout};
use crate::support::{Home, run_starting, within};
use miyu_ipc::connect_or_start;

#[tokio::test]
async fn the_base_system_is_removed_and_restored_by_its_group_name() {
    let home = Home::new();
    let root = home.root.path().to_path_buf();
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let cwd = home.dir.with_extension("group");
    std::fs::create_dir_all(&cwd).expect("建得了目录");
    let zh = "zh_CN.UTF-8";

    let nothing = run(&root, &cwd, zh, &["pkg", "install", "basesystem"]).await;
    assert_eq!(
        stdout(&nothing),
        "basesystem 里没有要动的包\n",
        "{nothing:?}"
    );

    let command = crate::support::cli::command(&root, &cwd, zh, &["pkg", "remove", "basesystem"]);
    let dir = cwd.clone();
    let removed = tokio::task::spawn_blocking(move || run_starting(&dir, command, "\n"))
        .await
        .expect("没 panic");
    let shown = stdout(&removed);
    assert!(
        shown.starts_with("将卸载 basesystem\n包含：") && shown.contains("shell"),
        "{shown}"
    );
    assert!(shown.contains("\n卸掉了：shell\n"), "{shown}");
    assert_eq!(removed.status.code(), Some(0), "{removed:?}");

    let back = run(&root, &cwd, zh, &["pkg", "install", "basesystem", "--yes"]).await;
    let shown = stdout(&back);
    assert!(shown.contains("装好了：shell\n"), "{back:?}");
    assert!(!shown.contains("将装回"), "--yes 不问：{shown}");
    drop(held);
    if let Err(error) = std::fs::remove_dir_all(&cwd) {
        eprintln!("临时目录没删掉：{error}");
    }
}
