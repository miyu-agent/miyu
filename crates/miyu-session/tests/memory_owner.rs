//! 照称呼写人（施工 R-6 再修，`docs/blueprint/memory.md` 第六条第 4 款、第七条第 2 款）：真会话、整理记忆的模型是一台假服务器，
//! 先交抽取的回答、再交合并的。
//!
//! 抽取：这个会话的快照带了「关于你」、设了称呼的，指令里照称呼写主人；没设的、只有空白的、快照不带的写「用户」那一句（配置里
//! 的称呼是主人的，不带「关于你」的会话不能把它当成那个人）。合并：照配置里的称呼写，不看是哪个会话起的。

use miyu_http::testkit::Server;
use miyu_session::testkit::{Play, Script};

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat};
use crate::support::*;

/// 抽取指令里写了称呼的那一句。
const NAMED: &str = "your owner as 阿凯.";
/// 抽取指令里没有称呼的那一句。
const UNNAMED: &str = "your owner as the user, translated into the language you write in.";
/// 合并指令里写了称呼的那一句。
const MERGE_NAMED: &str = "starting with your owner (write them as 阿凯),";
/// 合并指令里没有称呼的那一句。
const MERGE_UNNAMED: &str =
    "starting with your owner (write them as the user, translated into the language you write in),";

/// 配置里写着称呼 `name`。
fn called(name: &str) -> String {
    format!("[profile]\nname = \"{name}\"\n")
}

/// 抽一次、合一次（两轮，攒一个会话就合）：会话的「关于你」写称呼 `profile`，配置里另接 `personal`。交回抽取、合并的请求。
async fn asked_both(profile: &str, personal: &str) -> (String, String) {
    let server = Server::start(vec![every_turn("记一笔"), stream("{}")]).await;
    let mut home = Home::new();
    organizer_as(&mut home, &server, "merge_sessions = 1\n", personal);
    let lines = Lines {
        profile: miyu_policy::Profile {
            name: profile.to_string(),
            ..miyu_policy::Profile::default()
        },
        ..Lines::default()
    };
    let handle = home
        .create_full(
            &Script::new([Play::Says("好。"), Play::Says("好。")]),
            &catalog(&home),
            Opening::default(),
            lines,
        )
        .await;
    chat(&home, &handle, 1, "我养了一只猫").await;
    chat(&home, &handle, 2, "是只橘猫").await;
    requested(&server, 2).await;
    let both = (asked(&server, 0), asked(&server, 1));
    stop(&handle).await;
    both
}

#[tokio::test]
async fn the_owner_goes_by_the_name_they_set() {
    let (extract, merge) = asked_both("阿凯", &called("阿凯")).await;
    assert!(
        extract.starts_with("Find what is worth remembering"),
        "{extract}"
    );
    assert!(extract.contains(NAMED), "{extract}");
    assert!(!extract.contains("the owner"), "{extract}");
    assert!(merge.starts_with("Below are your memories"), "{merge}");
    assert!(merge.contains(MERGE_NAMED), "{merge}");
}

#[tokio::test]
async fn without_a_name_the_owner_is_the_user() {
    let (extract, merge) = asked_both("", "").await;
    assert!(extract.contains(UNNAMED), "{extract}");
    assert!(merge.contains(MERGE_UNNAMED), "{merge}");
    // 只有空白的当没设。
    let (extract, merge) = asked_both("  ", &called("  ")).await;
    assert!(extract.contains(UNNAMED), "{extract}");
    assert!(merge.contains(MERGE_UNNAMED), "{merge}");
}

#[tokio::test]
async fn a_session_without_the_profile_block_writes_the_user_but_the_summary_uses_the_name() {
    // 快照不带「关于你」的（群聊、桥没写 `profile` 的场所会话）：抽取不拿主人的称呼当这个人；合并写的是
    // 这一间的主人，照配置里的称呼。
    let (extract, merge) = asked_both("", &called("阿凯")).await;
    assert!(extract.contains(UNNAMED), "{extract}");
    assert!(merge.contains(MERGE_NAMED), "{merge}");
}

#[tokio::test]
async fn organizing_now_writes_the_name_too() {
    // 人叫的现在就整理（`Handle::dream`）走另一条路拼抽取的请求，照样照称呼写。
    let server = Server::start(vec![every_turn("记一笔"), stream("{}")]).await;
    let mut home = Home::new();
    organizer_as(&mut home, &server, "extract_turns = 5\n", &called("阿凯"));
    let lines = Lines {
        profile: miyu_policy::Profile {
            name: "阿凯".to_string(),
            ..miyu_policy::Profile::default()
        },
        ..Lines::default()
    };
    let handle = home
        .create_full(
            &Script::new([Play::Says("好。")]),
            &catalog(&home),
            Opening::default(),
            lines,
        )
        .await;
    chat(&home, &handle, 1, "我养了一只猫").await;
    handle.dream().await.expect("会话在跑").expect("整理了");
    assert!(asked(&server, 0).contains(NAMED), "{}", asked(&server, 0));
    assert!(
        asked(&server, 1).contains(MERGE_NAMED),
        "{}",
        asked(&server, 1)
    );
    stop(&handle).await;
}
