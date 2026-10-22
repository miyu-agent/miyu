//! 认出来（施工 E-1 补，`docs/blueprint/memory.md` 第三条第 8 款，设计 42 PN4）：真会话、真记忆日志。群里和她说话的那个人有人卡
//! 的，回合开始带上他的卡：在常驻摘要后面、这一轮的话前面，以后原样回放；一段上下文里一个人只带一次，换了人带新的那张，清空以后
//! 重新算。主人、没有卡的、卡作废了的不带。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::VenueMessage;
use miyu_kernel::id::{ExternalId, VenueId};
use miyu_kernel::origin::{By, External};
use miyu_kernel::request::{Message, Request};
use miyu_kernel::session::Command;
use miyu_recall::{Card, CardRetired, MemoryEvent, Saved};
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_store::recall::Room;

use crate::support::meaning::catalog;
use crate::support::*;

fn persona() -> Room {
    Room::persona(&alice_account(), "engineer")
}

fn group() -> VenueId {
    VenueId::parse("qq:group:1").expect("合写法")
}

/// 群里的 `id`；带 `account` 的是对应表认出来的主人。
fn member(id: &str, owner: bool) -> By {
    By::External(External {
        venue: group(),
        id: ExternalId::parse(id).expect("合写法"),
        account: owner.then(alice_account),
        role: None,
    })
}

fn write(home: &Home, event: &MemoryEvent) {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
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

/// 这一间记一条主人的事：常驻摘要那一块有东西。
fn remembered(home: &Home) {
    let saved = Saved {
        class: "user".into(),
        text: "主人养了一只猫".into(),
        sources: Vec::new(),
        audience: Vec::new(),
        replaces: None,
        about: None,
        venue: None,
        keywords: None,
        who: None,
    };
    write(home, &MemoryEvent::Saved(saved));
}

async fn session(home: &Home, script: &Script) -> Handle {
    let lines = Lines {
        venue: group(),
        group: true,
        ..Lines::default()
    };
    home.create_full(script, &catalog(home), Opening::default(), lines)
        .await
}

/// `by` 在群里说 `words`（命令编号 `cmd`），等这一轮结束。
async fn heard(handle: &Handle, cmd: &str, by: By, words: &str) {
    let mut pushes = watch(handle).await;
    let send = Command::Send {
        blocks: vec![Block::Text(Text {
            text: words.to_string(),
        })],
        urgent: false,
        venue: Some(VenueMessage {
            msg: cmd.to_string(),
            ..VenueMessage::default()
        }),
    };
    within("回应", handle.command(id(cmd), by, send))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 一次请求里人那一边的字块，照先后。
fn texts(request: &Request) -> Vec<String> {
    request
        .messages
        .iter()
        .flat_map(|message| match message {
            Message::User { blocks } => blocks.clone(),
            _ => Vec::new(),
        })
        .filter_map(|block| match block {
            Block::Text(Text { text }) => Some(text),
            _ => None,
        })
        .collect()
}

/// 请求里的人卡那几块，照先后。
fn cards(request: &Request) -> Vec<String> {
    texts(request)
        .into_iter()
        .filter(|text| text.starts_with("<person>"))
        .collect()
}

const XIAOLIN: &str =
    "<person>\nTalking to you: qq:20017 (小林, 林林)\n小林是群友，喜欢拍照。\n</person>\n";
const AJIE: &str = "<person>\nTalking to you: qq:20018\n阿杰用 RX 7800 XT。\n</person>\n";

#[tokio::test]
async fn the_one_talking_brings_their_card_once_in_a_stretch() {
    let home = Home::new();
    remembered(&home);
    card(
        &home,
        "qq:20017",
        "小林是群友，喜欢拍照。",
        &["小林", "林林"],
    );
    card(&home, "qq:20018", "阿杰用 RX 7800 XT。", &[]);
    let script = Script::new([
        Play::Says("在。"),
        Play::Says("嗯。"),
        Play::Says("好。"),
        Play::Says("哈。"),
        Play::Says("是。"),
    ]);
    let handle = session(&home, &script).await;
    let xiaolin = member("qq:20017", false);

    heard(&handle, "8801", xiaolin.clone(), "在吗").await;
    let first = &script.requests()[0].1;
    assert_eq!(cards(first), [XIAOLIN], "叫法接在是谁后面");
    let said = texts(first);
    let at = |prefix: &str| {
        said.iter()
            .position(|text| text.starts_with(prefix) || text.ends_with(prefix))
            .unwrap_or_else(|| panic!("有 {prefix}：{said:?}"))
    };
    assert!(
        at("<memories>") < at("<person>"),
        "在常驻摘要后面：{said:?}"
    );
    assert!(at("<person>") < at("在吗"), "在这一轮的话前面：{said:?}");

    heard(&handle, "8802", xiaolin.clone(), "今天拍了红叶").await;
    let second = &script.requests()[1].1;
    assert_eq!(cards(second), [XIAOLIN], "一段里只带一次，原样回放");
    assert!(
        texts(second).starts_with(&said),
        "以前的字一个不动（缓存不断）：{:?}",
        texts(second)
    );

    heard(&handle, "8803", member("qq:20018", false), "显卡好用").await;
    heard(&handle, "8804", xiaolin.clone(), "我也想换").await;
    let requests = script.requests();
    assert_eq!(cards(&requests[2].1), [XIAOLIN, AJIE], "换了人带新的那张");
    assert_eq!(cards(&requests[3].1), [XIAOLIN, AJIE], "回来的不再带");

    within(
        "清空",
        handle.command(id("cmd-clear"), alice(), Command::Clear),
    )
    .await
    .expect("会话在跑");
    heard(&handle, "8805", xiaolin, "还在吗").await;
    assert_eq!(
        cards(&script.requests()[4].1),
        [XIAOLIN],
        "清空以后重新算（压缩同理：都照最近一个检查点以后）"
    );
    stop(&handle).await;
}

#[tokio::test]
async fn no_card_for_the_owner_or_someone_without_a_live_one() {
    let home = Home::new();
    card(&home, "qq:10001", "这是主人。", &[]);
    card(&home, "qq:20019", "以前认识的。", &[]);
    let gone = CardRetired {
        who: "qq:20019".into(),
        why: "认错了".into(),
    };
    write(&home, &MemoryEvent::CardRetired(gone));
    let script = Script::new([Play::Says("在。"), Play::Says("嗯。"), Play::Says("好。")]);
    let handle = session(&home, &script).await;
    heard(&handle, "8801", member("qq:10001", true), "我来了").await;
    heard(&handle, "8802", member("qq:20019", false), "还记得我吗").await;
    heard(&handle, "8803", member("qq:20020", false), "新人报到").await;
    for (n, (_, request)) in script.requests().iter().enumerate() {
        assert!(cards(request).is_empty(), "第 {n} 轮：{:?}", texts(request));
    }
    stop(&handle).await;
}
