//! 每人一份的包的后台页（施工 IM-1 中下）：登记的方法照（包，账号）记，`package.call` 转给调的那个人那一份，没在跑的替他拉起、
//! 等它登记了方法再转。成员只碰得到他能用的每人一份的包的后台页：整台机器一份的、没勾「成员也能用」的回 `not_permitted`。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::id::AccountId;

use crate::support::extensions::*;
use crate::support::login::member;
use crate::support::*;

/// 包 `id` 替 `account` 的那一份记的文件。
fn log_of(home: &Home, account: &str, id: &str) -> PathBuf {
    let account = AccountId::parse(account).expect("合写法");
    home.root
        .account_dir(&account)
        .join("state")
        .join("packages")
        .join(id)
        .join("calls.log")
}

/// `path` 里带 `needle` 的行数。
fn count(path: &Path, needle: &str) -> usize {
    read(path)
        .lines()
        .filter(|line| line.contains(needle))
        .count()
}

/// 等到 `path` 里有带 `needle` 的一行。
async fn until_line(path: &Path, needle: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while count(path, needle) == 0 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {needle}：{}",
            read(path)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 装上 `id`：登记后台页的方法 `whoami`，接着答。`account` 是每人一份，`members` 勾了「成员也能用」。
fn install(home: &Home, program: &Program, id: &str, (account, members): (bool, bool)) {
    install_providing(
        home,
        id,
        &program.name(),
        "always",
        &steps(&[
            "record:calls.log",
            "hello",
            r#"ask:package.methods:{"methods":[{"name":"whoami"}]}"#,
            "serve",
        ]),
    );
    let manifest = home.root.system_packages().join(id).join("package.toml");
    let mut text = read(&manifest);
    if members {
        text = text.replacen("[package]\n", "[package]\nmembers = true\n", 1);
    }
    if account {
        text = text.replacen("[process]\n", "[process]\nscope = \"account\"\n", 1);
    }
    std::fs::write(&manifest, text).expect("写得进");
}

/// 调 `package` 的 `whoami`。
async fn whoami(client: &mut Client, id: &str, package: &str) -> Value {
    client
        .call(
            id,
            "package.call",
            json!({"package": package, "method": "whoami"}),
        )
        .await
}

#[tokio::test]
async fn each_person_s_page_reaches_their_own_copy() {
    let home = Home::new();
    let program = Program::new();
    install(&home, &program, "each", (true, true));
    install(&home, &program, "solo", (false, true));
    install(&home, &program, "kept", (true, false));
    let core = core_with_settings(&home, quick());
    let mine = log_of(&home, "alice", "each");
    let theirs = log_of(&home, "bob", "each");
    // 头一回装上：先替机主起一份。
    until_line(&mine, "\"methods\":1").await;

    let mut owner = Client::connect(Arc::clone(&core));
    owner.hello().await;
    let answered = whoami(&mut owner, "c-5d1f0a7e2b9c4386-1", "each").await;
    assert_eq!(
        answered["result"],
        json!({"answered": "whoami"}),
        "{answered}"
    );
    until_line(&mine, "method.call").await;
    assert!(!theirs.exists(), "没用过的人没有");

    // 成员调：替他起一份，转给他那一份。
    let mut bob = member(&core, "bob").await;
    let answered = whoami(&mut bob, "c-5d1f0a7e2b9c4386-2", "each").await;
    assert_eq!(
        answered["result"],
        json!({"answered": "whoami"}),
        "{answered}"
    );
    until_line(&theirs, "method.call").await;
    assert_eq!(count(&mine, "method.call"), 1, "他的调用没落到机主那一份上");

    // 整台机器一份的、没勾「成员也能用」的：成员碰不到；机主照常。
    for package in ["solo", "kept"] {
        let refused = whoami(&mut bob, "c-5d1f0a7e2b9c4386-3", package).await;
        assert_eq!(
            refused["error"]["data"]["reason"], "not_permitted",
            "{package}：{refused}"
        );
        let file = bob
            .call(
                "c-5d1f0a7e2b9c4386-4",
                "package.file",
                json!({"package": package, "path": "index.html"}),
            )
            .await;
        assert_eq!(
            file["error"]["data"]["reason"], "not_permitted",
            "{package}：{file}"
        );
    }
    core.stop_extensions().await;
}
