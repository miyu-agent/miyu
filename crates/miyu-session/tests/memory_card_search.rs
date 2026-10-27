//! 搜得到人卡（施工 E-3，设计 42 第三节第 4 条，`docs/blueprint/memory.md`「工具」）：真会话、真记忆日志，执行器替身照剧本调
//! `memory_search`。问「你认识小林吗」整句，对上叫法交回他的卡，排在记下的后面、以前的对话前面；平台编号、正文里的词也对得上；
//! 作废的卡不出，别的账号那一间的卡搜不到，没有卡的照旧。名字、编号是虚构的。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::AccountId;
use miyu_kernel::session::Command;
use miyu_recall::{Card, CardRetired, MemoryEvent, PERSON, Saved};
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_store::recall::Room;
use miyu_tool::Catalog;

use crate::support::*;

/// 基础系统加记忆的三件，和核心里一样。
fn catalog(home: &Home) -> Catalog {
    let mut tools = miyu_basesystem::tools(home.resources.path()).expect("读得出");
    tools.extend(miyu_memory::tools(home.resources.path()).expect("读得出"));
    Catalog::new(tools).expect("合写法")
}

fn write(home: &Home, event: &MemoryEvent) {
    let (log, _) = home
        .logs
        .open(&Room::persona(&alice_account(), "engineer"))
        .expect("开得了");
    log.append(now(), alice(), None, event).expect("记得下");
}

fn card(home: &Home, who: &str, text: &str, names: &[&str]) {
    let card = Card {
        who: who.into(),
        text: text.into(),
        names: names.iter().map(ToString::to_string).collect(),
    };
    write(home, &MemoryEvent::Card(card));
}

/// 这一间记一条关于 `who` 的事。
fn person(home: &Home, who: &str, text: &str) {
    let saved = Saved {
        class: PERSON.into(),
        text: text.into(),
        sources: Vec::new(),
        audience: Vec::new(),
        replaces: None,
        about: None,
        venue: None,
        keywords: None,
        who: Some(who.into()),
        people: Vec::new(),
    };
    write(home, &MemoryEvent::Saved(saved));
}

/// 小林（有一条记下的、一张卡）、阿杰（只有卡）、阿明（卡作废了）。
fn known(home: &Home) {
    person(home, "qq:20017", "小林用 RX 7800 XT");
    card(
        home,
        "qq:20017",
        "群里认识的摄影爱好者，周末常去海边拍照。\n下周搬去大阪。",
        &["小林", "林林"],
    );
    card(home, "qq:20018", "阿杰写 Rust，说话直但靠谱。", &[]);
    card(home, "qq:20019", "阿明爱打牌。", &["阿明"]);
    let retired = CardRetired {
        who: "qq:20019".into(),
        why: "认错人了".into(),
    };
    write(home, &MemoryEvent::CardRetired(retired));
}

/// 她照剧本搜一次 `query`，交回这次搜的结果。账号是 `account` 的那一间（没有的是 alice）。
async fn searched(home: &Home, query: &str, account: Option<AccountId>) -> String {
    let script = Script::new([
        Play::calls(&[(
            "memory_search",
            &serde_json::json!({ "query": query }).to_string(),
        )]),
        Play::Says("好。"),
    ]);
    let lines = Lines {
        memory_account: account.unwrap_or_else(alice_account),
        ..Lines::default()
    };
    let handle = home
        .create_full(&script, &catalog(home), Opening::default(), lines)
        .await;
    let log = chat(home, &handle, query).await;
    stop(&handle).await;
    last_result(&log)
}

/// 说 `words`，等这一轮结束，交回日志。
async fn chat(home: &Home, handle: &Handle, words: &str) -> Vec<Event> {
    let said = Command::Send {
        blocks: vec![Block::Text(Text {
            text: words.to_string(),
        })],
        urgent: false,
        venue: None,
    };
    within("回应", handle.command(id("cmd-1"), alice(), said))
        .await
        .expect("会话在跑");
    until_logged(home, handle.id(), |log| {
        log.iter()
            .any(|event| matches!(event.body, Body::TurnEnded(_)))
    })
    .await
}

/// 日志里最后一次调用的结果。
fn last_result(log: &[Event]) -> String {
    let result = log
        .iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .expect("有一次调用");
    match result.blocks.as_slice() {
        [Block::Text(Text { text })] => text.clone(),
        other => panic!("一段字：{other:?}"),
    }
}

const XIAOLIN: &str =
    "card of qq:20017 (小林, 林林): 群里认识的摄影爱好者，周末常去海边拍照。 下周搬去大阪。";

/// 验收的核心：「你认识小林吗」整句对上叫法，交回小林的卡，排在记下的那一条后面；别人的卡不混进来。
#[tokio::test]
async fn asking_whether_she_knows_someone_finds_their_card() {
    let home = Home::new();
    known(&home);
    let found = searched(&home, "你认识小林吗", None).await;
    let lines: Vec<&str> = found.lines().collect();
    assert_eq!(lines.len(), 2, "{found}");
    assert!(
        lines[0].starts_with("m1 person qq:20017 ") && lines[0].ends_with(": 小林用 RX 7800 XT"),
        "记下的在前：{found}"
    );
    assert_eq!(lines[1], XIAOLIN, "{found}");
}

/// 平台编号、只有卡的人、正文里的词都找得到；作废了的卡不出。
#[tokio::test]
async fn the_id_and_the_text_find_cards_and_retired_ones_stay_out() {
    let home = Home::new();
    known(&home);
    assert_eq!(
        searched(&home, "qq:20018 是谁", None).await,
        "card of qq:20018: 阿杰写 Rust，说话直但靠谱。"
    );
    assert_eq!(searched(&home, "谁常去海边拍照", None).await, XIAOLIN);
    // 另一个家目录：前面几次搜的那几轮进了回合库，会被当成以前的对话搜到。
    let home = Home::new();
    known(&home);
    assert_eq!(
        searched(&home, "阿明是谁", None).await,
        "Nothing found.",
        "作废的卡不出"
    );
}

/// 别的账号那一间搜不到这一间的卡（施工 Y-7：甲乙的记忆不串）；没有卡的照旧。
#[tokio::test]
async fn another_account_does_not_find_these_cards() {
    let home = Home::new();
    known(&home);
    let bob = AccountId::parse("bob").expect("合写法");
    assert_eq!(
        searched(&home, "你认识小林吗", Some(bob)).await,
        "Nothing found."
    );
    let home = Home::new();
    person(&home, "qq:20017", "小林用 RX 7800 XT");
    let found = searched(&home, "小林", None).await;
    assert!(
        found.starts_with("m1 person qq:20017 ") && !found.contains("card of"),
        "没有卡的只有记下的：{found}"
    );
}
