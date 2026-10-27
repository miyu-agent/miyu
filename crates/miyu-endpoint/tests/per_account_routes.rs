//! 每人一份的包的命令、搜索来源、回合开始的挂接点（施工 IM-1 中下三）：照会话属主（搜索照替谁搜）转给他那一份。命令、搜索用到时
//! 替他拉起；挂接点只问在跑的，不为它拉起。成员用了，三样都落在他那一份上，机主那一份一样都不收。

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::json;

use miyu_kernel::id::AccountId;
use miyu_session::testkit::{Play, Script};

use crate::support::extensions::*;
use crate::support::login::member;
use crate::support::*;

/// 包 `each` 替 `account` 的那一份记的文件。
fn log_of(home: &Home, account: &AccountId) -> PathBuf {
    home.root
        .account_dir(account)
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

/// 装上每人一份、成员也能用的 `each`：一件工具，一条谁都能用的命令 `/weather`，一个搜索来源 `web`，回合开始交一块提示。
fn install(home: &Home, program: &Program) {
    let tools = json!({"tools": [{"name": "ping", "description": "Ping.",
        "input_schema": {"type": "object"}, "access": "read", "venues": ["local"]}]});
    let capabilities = ["tools", "commands", "search", "context.inject"];
    install_asking(
        home,
        "each",
        &program.name(),
        "always",
        &steps(&[
            "record:calls.log",
            "hello",
            &format!("ask:provide:{tools}"),
            "inject:hint",
            "serve",
        ]),
        &capabilities,
    );
    approve(home, "each", &capabilities);
    let manifest = home
        .root
        .system_packages()
        .join("each")
        .join("package.toml");
    let text = read(&manifest)
        .replacen("[package]\n", "[package]\nmembers = true\n", 1)
        .replacen("[process]\n", "[process]\nscope = \"account\"\n", 1)
        + "\n[[slash]]\nname = \"weather\"\naccess = \"anyone\"\nsummary = { en = \"Weather\" }\n\n[[search]]\nid = \"web\"\nname = { en = \"Each\" }\n";
    std::fs::write(&manifest, text).expect("写得进");
}

#[tokio::test]
async fn a_member_s_commands_searches_and_hooks_reach_their_own_copy() {
    let home = Home::new();
    let program = Program::new();
    install(&home, &program);
    let core = core_with_script(&home, quick(), &Script::new([Play::Says("好。")]));
    let alice = AccountId::parse("alice").expect("合写法");
    let bob = AccountId::parse("bob").expect("合写法");
    let mine = log_of(&home, &alice);
    let theirs = log_of(&home, &bob);
    // 头一回装上：先替机主起一份登记工具。
    until_line(&mine, "\"tools\":1").await;

    let mut member = member(&core, "bob").await;
    let workspace = home.root.workspace(&bob);
    let session = member.create("c1", &workspace.to_string_lossy()).await;

    // 命令：替他拉起他那一份，转给它。
    let ran = member
        .call(
            "k-3b8e1f0c9a2d4765-1",
            "command.run",
            json!({"session": session, "text": "/weather tokyo"}),
        )
        .await;
    assert!(ran["result"].is_object(), "{ran}");
    until_line(&theirs, "command.call").await;

    // 回合开始的挂接点：他那一份在跑，问它。
    member.say("s1", &session, "你好").await;
    until_turns_in(&dir(&home, &bob, &session), 1).await;
    until_line(&theirs, "user.append").await;

    // 搜索来源：替他搜，问他那一份。
    let searched = miyu_endpoint::search_sources::query(
        &core,
        ("each/web", Some(&bob)),
        json!({"source": "each/web", "query": "rust", "want": 8}),
        Duration::from_secs(30),
    )
    .await;
    assert!(searched.is_ok(), "{searched:?}");
    until_line(&theirs, "search.query").await;

    for asked in ["command.call", "user.append", "search.query"] {
        assert_eq!(count(&mine, asked), 0, "{asked} 没落到机主那一份上");
    }
    core.stop_extensions().await;
}
