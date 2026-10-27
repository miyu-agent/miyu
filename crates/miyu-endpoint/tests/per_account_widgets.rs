//! 每人一份的包的界面块（施工 IM-1 中下五）：块照（包，账号）记。头订阅时「谁在看」只告诉看的那个人那一份，内容、运行状态
//! 照他那一份；别人那一份收不到，也不为看一眼替他拉起。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::id::AccountId;

use crate::support::extensions::*;
use crate::support::login::member;
use crate::support::*;

/// 包 `each` 替 `account` 的那一份记的文件。
fn log_of(home: &Home, account: &str) -> PathBuf {
    let account = AccountId::parse(account).expect("合写法");
    home.root
        .account_dir(&account)
        .join("state")
        .join("packages")
        .join("each")
        .join("calls.log")
}

/// `path` 里带 `needle` 的行数。
fn count(path: &Path, needle: &str) -> usize {
    read(path)
        .lines()
        .filter(|line| line.contains(needle))
        .count()
}

/// 等到 `path` 里带 `needle` 的行有 `want` 那么多。
async fn until_count(path: &Path, needle: &str, want: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while count(path, needle) < want {
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {want} 行 {needle}：{}",
            read(path)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 订界面块：底栏在屏上。
async fn subscribe(client: &mut Client, id: &str) -> Value {
    let request = json!({"jsonrpc": "2.0", "id": id, "method": "subscribe",
        "params": {"stream": "widgets", "showing": ["status"]}});
    client.line(&request.to_string()).await;
    client.until_reply(id).await.1
}

/// 回应里那一块。
fn temp(reply: &Value) -> Value {
    reply["result"]["widgets"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .find(|one| one["id"] == "temp")
        .cloned()
        .unwrap_or_else(|| panic!("没列出 temp：{reply}"))
}

#[tokio::test]
async fn each_person_s_head_watches_their_own_copy() {
    let home = Home::new();
    let program = Program::new();
    let capabilities = ["tools", "widgets"];
    install_asking(
        &home,
        "each",
        &program.name(),
        "always",
        &steps(&[
            "record:calls.log",
            "hello",
            // 先登记工具、再登记方法：方法一登记上核心就会转调用过来，测试扩展得已经在 `serve` 那一步。
            r#"ask:provide:{"tools":[]}"#,
            r#"ask:package.methods:{"methods":[{"name":"whoami"}]}"#,
            "serve",
        ]),
        &capabilities,
    );
    approve(&home, "each", &capabilities);
    let manifest = home
        .root
        .system_packages()
        .join("each")
        .join("package.toml");
    let text = read(&manifest)
        .replacen("[package]\n", "[package]\nmembers = true\n", 1)
        .replacen("[process]\n", "[process]\nscope = \"account\"\n", 1)
        + "\n[[widget]]\nid = \"temp\"\nplace = \"status\"\n";
    std::fs::write(&manifest, text).expect("写得进");
    let core = core_with_settings(&home, quick());
    let mine = log_of(&home, "alice");
    let theirs = log_of(&home, "bob");
    // 头一回装上：先替机主起一份。
    until_count(&mine, "\"methods\":1", 1).await;

    // 机主看：告诉的是他那一份。
    let mut owner = Client::connect(Arc::clone(&core));
    owner.hello().await;
    let reply = subscribe(&mut owner, "w1").await;
    assert_eq!(temp(&reply)["running"], true, "他那一份在跑：{reply}");
    until_count(&mine, "widget.watch", 1).await;
    // 他那一份推来的内容到了他的头上。
    loop {
        let next = owner.next().await.expect("没断开");
        if next["method"] == "widget.changed" && next["params"]["id"] == "temp" {
            let content = &next["params"]["widget"]["content"];
            if content.is_array() {
                assert_eq!(content[0]["text"], "watching", "{next}");
                break;
            }
        }
    }

    // 成员看：他那一份还没起，不为这一眼替他起；块照他那一份算，不在跑。
    let mut bob = member(&core, "bob").await;
    let reply = subscribe(&mut bob, "w2").await;
    assert_eq!(temp(&reply)["running"], false, "{reply}");
    assert!(!theirs.exists(), "看一眼不替他起");

    // 他用了一下（后台页）以后起来了：「谁在看」补给他那一份，机主那一份不多收。
    let answered = bob
        .call(
            "c-7e3a1b5d9c2f4086-1",
            "package.call",
            json!({"package": "each", "method": "whoami"}),
        )
        .await;
    assert_eq!(
        answered["result"],
        json!({"answered": "whoami"}),
        "{answered}"
    );
    until_count(&theirs, "widget.watch", 1).await;
    assert_eq!(count(&mine, "widget.watch"), 1, "别人的头不告诉机主那一份");
    // 他那一份推的内容到他头上，记在他那一份下（不是机主的）。
    loop {
        let next = bob.next().await.expect("没断开");
        if next["method"] == "widget.changed" && next["params"]["id"] == "temp" {
            let widget = &next["params"]["widget"];
            if widget["content"].is_array() {
                assert_eq!(widget["running"], true, "{next}");
                break;
            }
        }
    }
    core.stop_extensions().await;
}
