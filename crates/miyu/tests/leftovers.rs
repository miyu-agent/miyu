//! 核心起来时收拾上一次崩了留下的（发布前第二轮审查，施工 AE-7）：升级装上的包时两次改名之间被杀，`<编号>/` 没了、
//! 原来那一份还在 `.<编号>.old`，起来时放回去。认不出的名字一个都不碰。

use std::fs;

use miyu_ipc::connect_or_start;

use crate::support::{Home, hello, within};

/// 起一次核心，握个手，等它空闲了自己走：起来的那一段做完了。
async fn start_once(home: &Home) {
    let (connection, token) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let reply = hello(connection, &token).await;
    assert!(reply.get("result").is_some(), "{reply}");
    home.until_stopped().await;
}

#[tokio::test]
async fn an_upgrade_cut_off_between_the_renames_is_put_back_when_the_core_starts() {
    let home = Home::new();
    let packages = home.root.packages();
    // 原来那一份已经挪到 `.<编号>.old`，暂存的 `.<编号>.new` 还没换进去：核心就在这时被杀了。
    fs::create_dir_all(packages.join(".pudding.old")).expect("建得了");
    fs::write(packages.join(".pudding.old").join("package.toml"), "old").expect("写得进");
    fs::create_dir_all(packages.join(".pudding.new")).expect("建得了");
    fs::write(packages.join(".pudding.new").join("package.toml"), "new").expect("写得进");
    // 换好了、备份没删掉的：本体在，备份是多余的。
    fs::create_dir_all(packages.join("cake")).expect("建得了");
    fs::write(packages.join("cake").join("package.toml"), "cake").expect("写得进");
    fs::create_dir_all(packages.join(".cake.old")).expect("建得了");
    // 人放的、认不出的：不碰。
    fs::create_dir_all(packages.join(".pudding.bak")).expect("建得了");
    fs::create_dir_all(packages.join(".Not An Id.old")).expect("建得了");
    start_once(&home).await;
    assert_eq!(
        fs::read_to_string(packages.join("pudding").join("package.toml")).ok(),
        Some("old".to_string()),
        "原来那一份放回去了"
    );
    for gone in [".pudding.old", ".pudding.new", ".cake.old"] {
        assert!(
            !packages.join(gone).exists(),
            "{gone} 是上一次剩下的，该清掉"
        );
    }
    assert!(packages.join("cake").join("package.toml").is_file());
    for kept in [".pudding.bak", ".Not An Id.old"] {
        assert!(packages.join(kept).exists(), "{kept} 认不出，不该碰");
    }
}
