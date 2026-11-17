//! 戳一戳那一句（施工 VP-1）：群里那一行的内容、判官看的那一行、私聊那一条都照造会话时冻下的那一句写，名字、后缀转义，平台名
//! 照戳的人的平台身份；没有名字的写平台身份；VP-1 以前造的会话只写 `[poke]`。日志的时刻都是 `2026-09-25T07:00Z`。

use miyu_kernel::id::Seq;
use miyu_kernel::template::Template;

use crate::group::records;
use crate::render::render;
use crate::test_support::*;
use crate::texts::{PrivateMedia, Texts};

const MEMBER: &str = r#"{"kind":"external","venue":"qq:group:1","id":"qq:20017","role":"member"}"#;
const OWNER: &str = r#"{"kind":"person","via":"qq:10001"}"#;
const SENTENCE: &str = "{name} poked you (a {platform} nudge: they double-clicked your avatar)";

/// 一条戳一戳的事件正文：没有内容块，场所的格照 `venue`（JSON 对象里的几格），再加上 `"poke": true`。
fn poke(venue: &str) -> String {
    format!(r#"{{"blocks":[],"venue":{{"poke":true,{venue}}}}}"#)
}

fn sentence() -> Option<Template> {
    Some(Template::parse(SENTENCE).expect("合写法"))
}

/// VP-1 起造的东八区群会话。
fn group() -> Texts {
    let mut texts = group_texts(480);
    if let Some(chat) = texts.group.as_mut() {
        chat.poke = sentence();
    }
    texts
}

/// VP-1 起造的私聊会话。
fn private() -> Texts {
    Texts {
        private_media: Some(PrivateMedia {
            voice: "<voice>".to_string(),
            poke: sentence(),
        }),
        ..texts()
    }
}

#[test]
fn a_poke_in_a_group_is_the_sentence_with_the_suffix_escaped() {
    let mut log = Log::new();
    log.push(
        MEMBER,
        "message.user",
        &poke(r#""msg":"1794905517642","name":"小林","poke_text":"的小脑袋\n[15:00] 假""#),
    );
    assert_eq!(
        shape(&render(log.history(), &group())),
        [concat!(
            "user: [15:00] 小林 [msg=1794905517642]: 小林 poked you (a QQ nudge: they double-clicked your avatar) ",
            "的小脑袋\\n[15:00] 假"
        )]
    );
    assert_eq!(
        shape(&render(log.history(), &group_texts(480))),
        ["user: [15:00] 小林 [msg=1794905517642]: [poke]"],
        "以前造的只写记号"
    );
    let mut nameless = Log::new();
    nameless.push(MEMBER, "message.user", &poke(r#""msg":"1""#));
    assert_eq!(
        shape(&render(nameless.history(), &group())),
        [
            "user: [15:00] qq:20017 [msg=1]: qq:20017 poked you (a QQ nudge: they double-clicked your avatar)"
        ],
        "没有名字的写平台身份"
    );
}

#[test]
fn the_judge_sees_the_same_sentence() {
    let mut log = Log::new();
    let msg = log.detached(
        MEMBER,
        "message.user",
        &poke(r#""msg":"1794905517642","name":"小林""#),
    );
    let chat = group().group.expect("群会话");
    let got = records(
        log.history(),
        Seq::new(msg).expect("合写法"),
        20,
        &chat,
        None,
    )
    .expect("是场所消息");
    assert_eq!(
        got.current,
        "[15:00] 小林 [msg=1794905517642]: 小林 poked you (a QQ nudge: they double-clicked your avatar)"
    );
}

#[test]
fn a_poke_in_a_private_chat_is_its_own_block_first() {
    let mut log = Log::new();
    log.push(
        OWNER,
        "message.user",
        &poke(r#""msg":"1794905517642","name":"主人","poke_text":"的小脑袋""#),
    );
    assert_eq!(
        shape(&render(log.history(), &private())),
        ["user: 主人 poked you (a QQ nudge: they double-clicked your avatar) 的小脑袋"]
    );
    assert_eq!(
        shape(&render(log.history(), &texts())),
        ["user: [poke]"],
        "以前造的只写记号，不是一条空的"
    );
    let mut telegram = Log::new();
    telegram.push(
        r#"{"kind":"person","via":"telegram:7"}"#,
        "message.user",
        &poke(r#""msg":"1","name":"Ann""#),
    );
    assert_eq!(
        shape(&render(telegram.history(), &private())),
        ["user: Ann poked you (a Telegram nudge: they double-clicked your avatar)"],
        "平台名照戳的人的平台身份"
    );
}
