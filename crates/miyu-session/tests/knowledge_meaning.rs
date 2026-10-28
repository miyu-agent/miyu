//! 知识库照意思找（施工 K-2，`docs/blueprint/knowledge.md`「怎么走」第四条）：远程的 embedding 是假服务器（回的向量都一样，
//! 什么都挺像），执行器替身照剧本调 `kb_search`。关键词对不上的，头一次搜只走关键词、搜完在后台补这个库的向量，补上以后
//! 照意思找得到；关键词对得上的照旧头一次就找得到。`off` 的、那一家出错的只走关键词。

use std::time::Duration;

use miyu_http::testkit::{Reply, Server};
use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::AccountId;
use miyu_kernel::session::Command;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::remote_embedding::*;
use crate::support::*;

/// 基础系统加知识库的几件，照包登记。
fn catalog(home: &Home) -> Catalog {
    let resources = home.resources.path();
    Catalog::in_packages(vec![
        (
            miyu_tool::BASESYSTEM,
            miyu_basesystem::tools(resources).expect("读得出"),
        ),
        (
            miyu_knowledge::PACKAGE,
            miyu_knowledge::tools(resources).expect("读得出"),
        ),
    ])
    .expect("合写法")
}

/// 说 `words`，等到结束了 `turns` 轮，交回日志。
async fn chat(home: &Home, handle: &Handle, turns: usize, words: &str) -> Vec<Event> {
    let said = Command::Send {
        blocks: vec![Block::Text(Text {
            text: words.to_string(),
        })],
        urgent: false,
        venue: None,
    };
    within(
        "回应",
        handle.command(id(&format!("cmd-{turns}")), alice(), said),
    )
    .await
    .expect("会话在跑");
    until_logged(home, handle.id(), |log| {
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            >= turns
    })
    .await
}

/// 日志里最后一次调用的结果。
fn last_result(log: &[Event]) -> String {
    log.iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some(format!("{:?}", result.blocks)),
            _ => None,
        })
        .expect("有一次调用")
}

fn search(query: &str) -> Play {
    let args = serde_json::json!({ "query": query }).to_string();
    Play::calls(&[("kb_search", &args)])
}

/// `account` 的知识库 `notes` 里放一篇。
fn put(home: &Home, text: &str) {
    let dir = home.root.path().join("kb/notes");
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::write(dir.join("cat.md"), text).expect("写得进");
}

/// 等 `account` 的库 `notes` 的块都补上了 `MODEL` 的向量。
async fn filled(home: &Home, account: &AccountId) {
    let index = home.knowledge.index_of(account, "notes");
    for _ in 0..600 {
        if index.missing(MODEL, 0, 10).expect("读得了").is_empty() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("等不到补上向量");
}

#[tokio::test]
async fn a_chunk_is_found_by_meaning_once_its_vectors_are_filled() {
    let server = Server::start((0..20).map(|_| vector(true)).collect()).await;
    let mut home = Home::new();
    remote(&mut home, &server);
    put(&home, "团子的饭是鸡胸肉\n");
    let script = Script::new([
        search("猫吃什么"),
        Play::Says("没找到。"),
        search("猫吃什么"),
        Play::Says("找到了。"),
    ]);
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    let first = last_result(&chat(&home, &handle, 1, "猫吃什么").await);
    assert!(
        first.contains("Nothing found."),
        "头一次只走关键词：{first}"
    );
    filled(&home, &alice_account()).await;
    let second = last_result(&chat(&home, &handle, 2, "再找找").await);
    assert!(
        second.contains("团子的饭是鸡胸肉"),
        "照意思找得到：{second}"
    );
    assert!(
        server
            .received()
            .iter()
            .any(|request| request.path == "/v1/embeddings"),
        "问句、块都照远程的算"
    );
}

#[tokio::test]
async fn keywords_still_find_it_the_first_time() {
    let server = Server::start((0..20).map(|_| vector(true)).collect()).await;
    let mut home = Home::new();
    remote(&mut home, &server);
    put(&home, "团子的饭是鸡胸肉\n");
    let script = Script::new([search("鸡胸肉"), Play::Says("找到了。")]);
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    let found = last_result(&chat(&home, &handle, 1, "鸡胸肉").await);
    assert!(found.contains("团子的饭是鸡胸肉"), "{found}");
}

#[tokio::test]
async fn off_means_keywords_only_and_nobody_is_asked() {
    let server = Server::start((0..20).map(|_| vector(true)).collect()).await;
    let mut home = Home::new();
    remote(&mut home, &server);
    home.configs = crate::support::routing::configs("[models]\nembedding = \"off\"\n", &[]);
    put(&home, "团子的饭是鸡胸肉\n");
    let script = Script::new([
        search("猫吃什么"),
        Play::Says("没找到。"),
        search("鸡胸肉"),
        Play::Says("找到了。"),
    ]);
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    let first = last_result(&chat(&home, &handle, 1, "猫吃什么").await);
    assert!(first.contains("Nothing found."), "{first}");
    tokio::time::sleep(Duration::from_millis(500)).await;
    let index = home.knowledge.index_of(&alice_account(), "notes");
    assert_eq!(
        index.missing(MODEL, 0, 10).expect("读得了").len(),
        1,
        "不补"
    );
    let second = last_result(&chat(&home, &handle, 2, "鸡胸肉").await);
    assert!(second.contains("团子的饭是鸡胸肉"), "关键词照旧：{second}");
    assert!(server.received().is_empty(), "不问那一家");
}

#[tokio::test]
async fn a_failing_remote_still_finds_by_keywords() {
    let down = Reply::error(500, &[], r#"{"error":{"message":"down"}}"#);
    let server = Server::start(vec![down; 20]).await;
    let mut home = Home::new();
    remote(&mut home, &server);
    put(&home, "团子的饭是鸡胸肉\n");
    let script = Script::new([search("鸡胸肉"), Play::Says("找到了。")]);
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    let found = last_result(&chat(&home, &handle, 1, "鸡胸肉").await);
    assert!(found.contains("团子的饭是鸡胸肉"), "{found}");
    assert!(!server.received().is_empty(), "问过那一家");
}

/// 账号 `account` 的账号日志里记了几笔用量。
fn billed(home: &Home) -> usize {
    std::fs::read_to_string(home.root.path().join(miyu_store::journal::FILE))
        .unwrap_or_default()
        .lines()
        .filter(|line| line.contains(r#""kind":"usage.oneshot""#))
        .count()
}

#[tokio::test]
async fn the_memory_account_s_base_is_filled_and_the_owner_pays() {
    // 记忆的账号是管理员、会话的属主是 alice：查、补的是管理员的库，远程的花费记在 alice 名下（同记忆）。
    let server = Server::start((0..20).map(|_| vector(true)).collect()).await;
    let mut home = Home::new();
    remote(&mut home, &server);
    let admin = AccountId::parse("admin").expect("合写法");
    put(&home, "团子的饭是鸡胸肉\n");
    let script = Script::new([
        search("猫吃什么"),
        Play::Says("没找到。"),
        search("猫吃什么"),
        Play::Says("找到了。"),
    ]);
    let lines = Lines {
        memory_account: admin.clone(),
        ..Lines::default()
    };
    let handle = home
        .create_full(&script, &catalog(&home), Opening::default(), lines)
        .await;
    chat(&home, &handle, 1, "猫吃什么").await;
    filled(&home, &admin).await;
    let second = last_result(&chat(&home, &handle, 2, "再找找").await);
    assert!(second.contains("团子的饭是鸡胸肉"), "{second}");
    assert!(billed(&home) > 0, "记在主人名下");
}
