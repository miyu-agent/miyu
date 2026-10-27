//! 每人一份的包的后台页（施工 IM-1 中下）：登记的方法照（包，账号）记，`package.call` 转给调的那个人那一份，没在跑的替他拉起、
//! 等它登记了方法再转。成员只碰得到他能用的每人一份的包的后台页：整台机器一份的、没勾「成员也能用」的回 `not_permitted`，给他的
//! 软件包列表里也不带 `page`（施工 IM-1 中下一补）。扩展的状态照看的人那一份报，管理员看得到每一份；关掉的时候每一份都停
//! （施工 IM-1 中下四）。

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

/// 装上 `id`：带后台页，登记后台页的方法 `whoami`，接着答。`account` 是每人一份，`members` 勾了「成员也能用」。
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
    std::fs::write(&manifest, text + "\n[page]\ndir = \"page\"\n").expect("写得进");
    let page = home.root.system_packages().join(id).join("page");
    std::fs::create_dir_all(&page).expect("建得了");
    std::fs::write(page.join("index.html"), "<p>hi</p>").expect("写得进");
}

/// `client` 拿到的软件包列表里，带 `page` 的那几个包。
async fn with_pages(client: &mut Client, id: &str) -> Vec<String> {
    let listed = client.call(id, "package.list", json!({})).await;
    let mut pages: Vec<String> = listed["result"]["packages"]
        .as_array()
        .expect("列表")
        .iter()
        .filter(|item| item["page"] == true)
        .filter_map(|item| item["package"].as_str().map(str::to_string))
        .collect();
    pages.sort();
    pages
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

    // 后台页：机主三个都列；成员只列他打得开的那一个，读得到它的文件。
    assert_eq!(
        with_pages(&mut owner, "c-5d1f0a7e2b9c4386-5").await,
        ["each", "kept", "solo"]
    );
    assert_eq!(with_pages(&mut bob, "c-5d1f0a7e2b9c4386-6").await, ["each"]);
    let file = bob
        .call(
            "c-5d1f0a7e2b9c4386-7",
            "package.file",
            json!({"package": "each", "path": "index.html"}),
        )
        .await;
    assert_eq!(file["result"]["eof"], true, "{file}");

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

    // 扩展的状态（施工 IM-1 中下四）：每人一份的照看的人那一份报，管理员另带拉起过的每一份。
    let each = status(&mut owner, "each").await;
    assert_eq!(each["scope"], "account", "{each}");
    assert_eq!(each["state"], "running", "机主那一份：{each}");
    let copies = |each: &Value| -> Vec<(String, String)> {
        each["copies"]
            .as_array()
            .unwrap_or_else(|| panic!("{each}"))
            .iter()
            .map(|copy| {
                (
                    copy["account"].as_str().unwrap_or_default().to_string(),
                    copy["state"].as_str().unwrap_or_default().to_string(),
                )
            })
            .collect()
    };
    assert_eq!(
        copies(&each),
        [
            ("alice".to_string(), "running".to_string()),
            ("bob".to_string(), "running".to_string())
        ]
    );
    let solo = status(&mut owner, "solo").await;
    assert!(
        solo.get("scope").is_none() && solo.get("copies").is_none(),
        "{solo}"
    );

    // 关掉：每一份都停（中上欠的那条）。
    let off = owner
        .call(
            "c-5d1f0a7e2b9c4386-8",
            "extension.disable",
            json!({"package": "each"}),
        )
        .await;
    assert!(off["result"].is_object(), "{off}");
    let each = status(&mut owner, "each").await;
    assert!(
        copies(&each).iter().all(|(_, state)| state == "off"),
        "{each}"
    );
    core.stop_extensions().await;
}
