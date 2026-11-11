//! 模型在回答后面编出下一轮的，截掉（施工 FT-1，`docs/blueprint/kernel/session.md`「截掉模型编的下一轮」）。真内核、真组装器照
//! 线上的样子跑：群里的人说一句（旁听记下），桥照它开一轮、带两块事实；假模型回的是 2026-11-11 线上那一段原样：第一行是真的回答，
//! 后面一个 `user` 接着照这一次请求里人这一边的样子编的一整条「主人说的话」。
//!
//! - 落下的 `message.assistant` 只有第一行，`model.called` 记着截在哪；
//! - 下一轮的请求里看不到编的那一段；
//! - 同一份剧本跑两遍日志一字不差，中途照日志重新载入再接着跑，下一轮的请求也一字不差；
//! - 只编了群里那一行（没有标签）的照样截；
//! - 正常的回答一个字节不动，`model.called` 没有那一格。

use crate::support::{group_texts, lines, policy, stage_with};
use miyu_assemble::{DefaultAssembler, Stable};
use miyu_kernel::block::Block;
use miyu_kernel::event::{
    Body, ContextInjected, ForgedCut, ForgedRole, MessageAssistant, VenueMessage,
};
use miyu_kernel::id::FactKind;
use miyu_kernel::origin::By;
use miyu_kernel::request::Message;
use miyu_kernel::session::Policy;
use miyu_kernel::testkit::{Line, Stage};

/// 线上那一条 `message.assistant` 的正文，原样（脱敏以后的）。
const SCENE: &str = "他骂我垃圾 按群规直接禁了\n\nuser<style-note>In this group a message is usually a few words, rarely more than a dozen characters, and a sticker often stands in for words.</style-note>\n<another-line>You are replying to msg=-1604144125 only. Other messages are handled separately.</another-line>\n[18:31] Owner (id=qq:10001, owner) [msg=-1604144125]: 直接踢了或者验证一下\n  reply-to: msg=-1488029772\n  @mentions: [you]";

/// 她真说的那一句。
const SAID: &str = "他骂我垃圾 按群规直接禁了";

/// 编出来的那一句「主人的话」。
const FORGED: &str = "直接踢了或者验证一下";

const STYLE: &str = "<style-note>In this group a message is usually a few words, rarely more than a dozen characters, and a sticker often stands in for words.</style-note>";

const MEMBER: &str = r#"{"kind":"external","venue":"qq:group:1","id":"qq:20017","role":"member"}"#;

/// 群会话的策略：同探针的那一份，组装器带着群会话的字（群里的人说的话渲染成一行一条），稳定区只有一句 system。
fn group_policy() -> Policy {
    let stable = Stable {
        tools: Vec::new(),
        system: "You are chatting in a group.".to_string(),
        demos: Vec::new(),
    };
    Policy {
        assembler: std::sync::Arc::new(DefaultAssembler::new(stable, group_texts())),
        ..policy()
    }
}

/// 一个群会话的替身。
fn group_stage() -> Stage {
    stage_with(group_policy)
}

/// 群里的人说 `words`（平台编号 `msg`，@ 了她、带引用），桥照它开一轮：两块事实，同线上的那两块。
fn heard(s: &mut Stage, msg: &str, words: &str) {
    let by: By = serde_json::from_str(MEMBER).expect("外部身份合写法");
    let venue = VenueMessage {
        msg: msg.to_string(),
        reply_to: Some("-1488029772".to_string()),
        name: Some("小林".to_string()),
        mentions_me: true,
        show_ids: true,
        ..VenueMessage::default()
    };
    let seq = s.overhear_from(by, venue, words);
    let fact = |kind: &str, text: String| ContextInjected {
        kind: FactKind::parse(kind).expect("类别合写法"),
        text,
        refs: Vec::new(),
        after: false,
    };
    s.respond_with(
        &[seq],
        vec![
            fact("style", STYLE.to_string()),
            fact(
                "line",
                format!(
                    "<another-line>You are replying to msg={msg} only. Other messages are handled separately.</another-line>"
                ),
            ),
        ],
    );
}

/// 第一轮模型回 `reply`，第二轮回一句平常的。
fn script(reply: &str) -> Stage {
    let mut s = group_stage();
    s.model([Line::says(reply), Line::says("嗯。")]);
    heard(&mut s, "1605385820", "他骂你垃圾");
    heard(&mut s, "1605385821", "然后呢");
    s
}

fn replies(s: &Stage) -> Vec<&MessageAssistant> {
    s.log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::MessageAssistant(reply) => Some(reply),
            _ => None,
        })
        .collect()
}

fn text_of(reply: &MessageAssistant) -> Vec<&str> {
    reply
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect()
}

/// 请求里人这一边的字，连在一起。
fn user_words(s: &Stage, request: usize) -> String {
    s.requests()[request]
        .1
        .messages
        .iter()
        .filter_map(|message| match message {
            Message::User { blocks } => Some(blocks),
            _ => None,
        })
        .flatten()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_scene_request_looks_like_the_one_on_site() {
    let s = script(SCENE);
    let words = user_words(&s, 0);
    assert!(words.contains(STYLE), "{words}");
    assert!(words.contains("<another-line>You are replying to msg=1605385820 only."));
    assert!(
        words.contains(
            "[15:00] 小林 (id=qq:20017) [msg=1605385820]: 他骂你垃圾\n  reply-to: msg=-1488029772\n  @mentions: [you]"
        ),
        "群里那一行：{words}"
    );
}

#[test]
fn the_forged_turn_is_cut_before_it_reaches_the_log() {
    let s = script(SCENE);
    let replies = replies(&s);
    assert_eq!(text_of(replies[0]), [SAID], "落下的只有她真说的那一行");
    assert!(!replies[0].interrupted);
    // `model.called` 记着截在哪：第 0 块，从 `user` 那一行的行首起，到原文的末尾。
    let at = format!("{SAID}\n\n").len();
    assert_eq!(
        s.model_calls()[0].cut,
        [ForgedCut {
            block: 0,
            at: at as u64,
            bytes: (SCENE.len() - at) as u64,
            role: ForgedRole::User,
        }]
    );
    assert!(s.model_calls()[1].cut.is_empty(), "第二轮平常的回答没有");
    // 截掉的原文不进日志。
    assert!(!lines(&s).join("\n").contains(FORGED));
}

#[test]
fn the_next_request_never_sees_the_forged_turn() {
    let s = script(SCENE);
    assert_eq!(s.requests().len(), 2);
    let next = format!("{:?}", s.requests()[1].1);
    assert!(next.contains(SAID), "她真说的那一句在：{next}");
    assert!(!next.contains(FORGED), "编的那一句不在：{next}");
    assert!(!next.contains("-1604144125"), "编的编号不在：{next}");
}

#[test]
fn the_same_script_writes_the_same_log_and_reloading_changes_nothing() {
    let (a, b) = (script(SCENE), script(SCENE));
    assert_eq!(lines(&a), lines(&b), "同样的输入，同样的日志");
    // 第一轮落了盘以后照日志重新载入，再接着跑第二轮：请求一字不差。
    let mut c = group_stage();
    c.model([Line::says(SCENE), Line::says("嗯。")]);
    heard(&mut c, "1605385820", "他骂你垃圾");
    c.crash();
    heard(&mut c, "1605385821", "然后呢");
    assert_eq!(
        c.requests()[1].1.canonical_bytes(),
        a.requests()[1].1.canonical_bytes(),
        "载入以后看到的也是截过的"
    );
}

#[test]
fn a_forged_group_line_alone_is_cut_too() {
    let reply =
        format!("{SAID}\nuser\n[18:31] Owner (id=qq:10001, owner) [msg=-1604144125]: {FORGED}");
    let s = script(&reply);
    assert_eq!(text_of(replies(&s)[0]), [SAID]);
    assert_eq!(s.model_calls()[0].cut.len(), 1);
    // 没写平台身份的那一种行首也认。
    let reply = format!("{SAID}\n\nHuman: [18:31] Owner [msg=-1604144125]: {FORGED}");
    let s = script(&reply);
    assert_eq!(text_of(replies(&s)[0]), [SAID]);
    assert_eq!(s.model_calls()[0].cut[0].role, ForgedRole::Human);
}

#[test]
fn an_ordinary_reply_is_left_alone() {
    for reply in [
        "好的，按群规处理。",
        "示例对话：\nuser: 你好\nassistant: 你好呀",
        "标签长这样：\n<style-note>…</style-note>\n它前面没有角色词。",
        "user\n<b>请求里没有这个标签</b>",
        "记录长这样：\n[18:31] Owner (id=qq:10001, owner) [msg=1]: 前面没有角色词",
        "末尾的空白也不动。\n\n",
    ] {
        let s = script(reply);
        assert_eq!(text_of(replies(&s)[0]), [reply], "一个字节不动");
        assert!(s.model_calls()[0].cut.is_empty(), "{reply}");
    }
}
