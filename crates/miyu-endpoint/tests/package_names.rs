//! 包的编号不再占账号名（施工 SU-4 中：包不再各有一个账号）：和主人的账号同名的包照常装得上；同一个包再装一次（升级）照常。

use serde_json::json;

use crate::support::login::*;
use crate::support::*;

/// 一个带进程的包的清单。
fn manifest(id: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"{id}\" }}\n\n[command]\nname = \"{id}\"\nprogram = \"miyu-xtool\"\nabout = {{ en = \"X\" }}\n\n[process]\n"
    )
}

/// 在工作目录里放一个包文件夹，交回路径。
fn folder(home: &Home, id: &str) -> String {
    let dir = home.work.join(id);
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::write(dir.join("package.toml"), manifest(id)).expect("写得进");
    dir.to_str().expect("UTF-8").to_string()
}

#[tokio::test]
async fn a_package_named_like_the_owner_installs_and_upgrades_itself() {
    let home = Home::new();
    let core = home.core(&miyu_session::testkit::Script::new([]));
    let mut owner = local(&core).await;
    let named = owner
        .call(
            "p1",
            "package.install",
            json!({"path": folder(&home, "alice")}),
        )
        .await;
    assert!(named.get("error").is_none(), "和主人同名的照装：{named}");
    for id in ["p2", "p3"] {
        let installed = owner
            .call(id, "package.install", json!({"path": folder(&home, "kit")}))
            .await;
        assert!(
            installed.get("error").is_none(),
            "装了再装是升级：{installed}"
        );
    }
}
