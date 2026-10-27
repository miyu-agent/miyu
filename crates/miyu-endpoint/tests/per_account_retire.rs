//! 扩展自己退、常驻（施工 IM-1 下，接入通讯平台设计 06 第 4.6 节第 1 样）：每人一份的那一份问过 `process.retire`、核心答应了、
//! 退出码 0 的记成停下（`retired`），不重启；再用到时替他起一份，答应以后、退干净以前来的调用等它退了交给新的那一份。说了
//! `process.keep` 的，核心再起来时替他拉起；没说的、说了又撤了的、停用了的人不起。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use miyu_kernel::id::AccountId;

use crate::support::extensions::*;
use crate::support::login::member;
use crate::support::*;

/// 包 `each` 替 `account` 的那一份的工作目录，不建（建了家目录的名字就算被占了）。
fn dir_of(home: &Home, account: &str) -> PathBuf {
    let account = AccountId::parse(account).expect("合写法");
    home.root
        .account_dir(&account)
        .join("state")
        .join("packages")
        .join("each")
}

/// 在 `dir`（建好）里放一个标记文件 `name`。
fn mark(dir: &Path, name: &str) {
    std::fs::create_dir_all(dir).expect("建得了");
    std::fs::write(dir.join(name), "").expect("写得进");
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

/// 装上每人一份、成员也能用的 `each`：登记了工具，做 `extra` 那几步（一般是工作目录里有标记文件才做的），登记后台页的方法
/// `whoami`，接着答。
fn install(home: &Home, program: &Program, extra: &[&str]) {
    // 那几步放在登记方法以前：方法一登记上核心就会转调用过来，测试扩展得已经在 `serve` 那一步。
    let mut all = vec!["record:calls.log", "hello", r#"ask:provide:{"tools":[]}"#];
    all.extend_from_slice(extra);
    all.extend([
        r#"ask:package.methods:{"methods":[{"name":"whoami"}]}"#,
        "serve",
    ]);
    install_providing(home, "each", &program.name(), "always", &steps(&all));
    let manifest = home
        .root
        .system_packages()
        .join("each")
        .join("package.toml");
    let text = read(&manifest)
        .replacen("[package]\n", "[package]\nmembers = true\n", 1)
        .replacen("[process]\n", "[process]\nscope = \"account\"\n", 1);
    std::fs::write(&manifest, text).expect("写得进");
}

/// `client` 调 `each` 的 `whoami`，答得上。
async fn whoami(client: &mut Client, id: &str) {
    let answered = client
        .call(
            id,
            "package.call",
            json!({"package": "each", "method": "whoami"}),
        )
        .await;
    assert_eq!(
        answered["result"],
        json!({"answered": "whoami"}),
        "{answered}"
    );
}

/// 管理员看 `each` 拉起过的每人一份是哪几个账号。
async fn copies(owner: &mut Client) -> Vec<String> {
    let each = status(owner, "each").await;
    each["copies"]
        .as_array()
        .unwrap_or_else(|| panic!("{each}"))
        .iter()
        .filter_map(|copy| copy["account"].as_str().map(str::to_string))
        .collect()
}

#[tokio::test]
async fn a_retired_copy_stays_stopped_and_comes_back_when_used() {
    let home = Home::new();
    let program = Program::new();
    install(&home, &program, &["if:retire.flag:retire"]);
    let mine = dir_of(&home, "alice");
    mark(&mine, "retire.flag");
    let log = mine.join("calls.log");
    // 头一回装上替机主起一份：登记完说要退，核心答应了。
    let core = core_with_settings(&home, quick());
    let mut owner = Client::connect(Arc::clone(&core));
    owner.hello().await;
    let stopped = until_state(&mut owner, "each", |one| one["state"] == "stopped").await;
    assert_eq!(stopped["reason"], "retired", "{stopped}");
    assert_eq!(stopped["failures"], 0, "不算失败：{stopped}");
    assert_eq!(count(&log, r#""ok":true"#), 1, "{}", read(&log));
    // 不重启：过了退避的时候照旧停着，只起过一回。
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(status(&mut owner, "each").await["state"], "stopped");
    assert_eq!(count(&log, "cwd:"), 1);

    // 再用到：替他起一份，交给新的那一份。
    std::fs::remove_file(mine.join("retire.flag")).expect("删得掉");
    whoami(&mut owner, "c-4f1d8a2e6b9c3057-1").await;
    assert_eq!(count(&log, "cwd:"), 2);
    core.stop_extensions().await;
}

#[tokio::test]
async fn a_call_that_comes_while_it_retires_goes_to_the_next_copy() {
    let home = Home::new();
    let program = Program::new();
    install(&home, &program, &["if:retire.flag:retire"]);
    let mine = dir_of(&home, "alice");
    mark(&mine, "retire.flag");
    mark(&mine, "linger");
    let log = mine.join("calls.log");
    let core = core_with_settings(&home, quick());
    // 答应了，还没退：照旧在跑。
    until_count(&log, r#""ok":true"#, 1).await;
    let mut owner = Client::connect(Arc::clone(&core));
    owner.hello().await;
    assert_eq!(status(&mut owner, "each").await["state"], "running");
    // 这时来的调用不发给它，等它退了交给新的那一份。
    std::fs::remove_file(mine.join("retire.flag")).expect("删得掉");
    let mut caller = Client::connect(Arc::clone(&core));
    caller.hello().await;
    let calling = tokio::spawn(async move {
        whoami(&mut caller, "c-2d7e9f1a4b6c8053-1").await;
    });
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!calling.is_finished(), "等它退");
    assert_eq!(count(&log, "method.call"), 0, "没发给要退的那一份");
    std::fs::remove_file(mine.join("linger")).expect("删得掉");
    calling.await.expect("答得上");
    assert_eq!(count(&log, "cwd:"), 2);
    assert_eq!(count(&log, "method.call"), 1);
    core.stop_extensions().await;
}

#[tokio::test]
async fn kept_copies_start_with_the_core_and_others_wait_to_be_used() {
    let home = Home::new();
    let program = Program::new();
    install(
        &home,
        &program,
        &[
            r#"if:keep.flag:ask:process.keep:{"on":true}"#,
            r#"if:unkeep.flag:ask:process.keep:{"on":false}"#,
        ],
    );
    let mine = dir_of(&home, "alice");
    mark(&mine, "keep.flag");
    let log = mine.join("calls.log");
    let core = core_with_settings(&home, quick());
    until_count(&log, r#""on":true"#, 1).await;
    let mut owner = Client::connect(Arc::clone(&core));
    owner.hello().await;
    // 两个成员用了一下：carol 没说要常驻；bob 说了，后来被停用了。
    let mut carol = member(&core, "carol").await;
    whoami(&mut carol, "c-9b2e5d7a1c3f4806-1").await;
    let mut bob = member(&core, "bob").await;
    let bobs = dir_of(&home, "bob");
    mark(&bobs, "keep.flag");
    whoami(&mut bob, "c-6a3c1e8f2d5b7049-1").await;
    until_count(&bobs.join("calls.log"), r#""on":true"#, 1).await;
    let off = owner
        .call("d", "account.disable", json!({"account": "bob"}))
        .await;
    assert_eq!(off["result"]["disabled"], true, "{off}");
    core.stop_extensions().await;
    drop((carol, bob, owner));
    drop(core);

    // 再起来：只有机主那一份照常驻名单起。这一回它说不常驻了。
    mark(&mine, "unkeep.flag");
    let core = core_with_settings(&home, quick());
    let mut owner = Client::connect(Arc::clone(&core));
    owner.hello().await;
    assert_eq!(copies(&mut owner).await, ["alice"]);
    until_count(&log, r#""on":false"#, 1).await;
    core.stop_extensions().await;
    drop(owner);
    drop(core);

    // 再起来：谁都不起，等用到。
    let core = core_with_settings(&home, quick());
    let mut owner = Client::connect(Arc::clone(&core));
    owner.hello().await;
    assert!(copies(&mut owner).await.is_empty());
    assert_eq!(count(&log, "cwd:"), 2);
    core.stop_extensions().await;
}
