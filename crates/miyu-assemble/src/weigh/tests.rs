//! 一条事件进请求以后写成的块（施工 CT-2）：群里旁听的一句是近况里那一行（带行首、名字、编号），睡着时收到的、没有近况的
//! 字的、私聊里旁听的是空的；开一轮的那一句是群里的一行；终端里人说的话照原样；人用 `!` 跑的命令、别的事件不管。
//! 写出来的字和真正渲染进请求的那一行一字不差。日志的时刻都是 `2026-09-25T07:00Z`，东八区是 15:00。

use miyu_kernel::block::Block;
use miyu_kernel::event::Event;

use super::rendered;
use crate::render::render;
use crate::test_support::*;
use crate::texts::Texts;

const MEMBER: &str = r#"{"kind":"external","venue":"qq:group:1","id":"qq:20017","role":"member"}"#;

/// 小林旁听的一句：平台的格另加 `fields`（JSON 对象里的几格，前面带逗号）。
fn ambient(log: &mut Log, text: &str, msg: &str, fields: &str) -> u64 {
    log.detached(
        MEMBER,
        "message.user",
        &format!(
            r#"{{"blocks":[{}],"venue":{{"msg":"{msg}","name":"小林","ambient":true,"show_ids":true{fields}}}}}"#,
            text_json(text)
        ),
    )
}

/// 小林说一句，开一轮。
fn opens(log: &mut Log, text: &str, msg: &str) -> u64 {
    let said = log.push(
        MEMBER,
        "message.user",
        &format!(
            r#"{{"blocks":[{}],"venue":{{"msg":"{msg}","name":"小林","show_ids":true}}}}"#,
            text_json(text)
        ),
    );
    log.start(said);
    said
}

/// 第 `seq` 条事件。
fn event(log: &Log, seq: u64) -> &Event {
    log.history()
        .events()
        .iter()
        .find(|event| event.seq.get() == seq)
        .expect("有这一条")
}

/// 第 `seq` 条照 `texts` 写成的字，一块一段。
fn words(log: &Log, seq: u64, texts: &Texts) -> Option<Vec<String>> {
    rendered(log.history(), event(log, seq), texts).map(|blocks| {
        blocks
            .iter()
            .map(|block| match block {
                Block::Text(text) => text.text.clone(),
                other => format!("{other:?}"),
            })
            .collect()
    })
}

#[test]
fn an_overheard_line_counts_its_clock_name_id_and_number_not_just_the_words() {
    let mut log = Log::new();
    let heard = ambient(&mut log, "早", "8801", "");
    opens(&mut log, "在吗", "8802");
    let texts = group_texts(480);
    let line = "[15:00] 小林 (id=qq:20017) [msg=8801]: 早\n";
    assert_eq!(words(&log, heard, &texts), Some(vec![line.to_string()]));
    // 和渲染进请求的那一行一字不差。
    let request = shape(&render(log.history(), &texts)).join("\n");
    assert!(request.contains(line), "{request}");
}

#[test]
fn the_line_that_opens_a_turn_is_the_group_line() {
    let mut log = Log::new();
    let said = opens(&mut log, "在吗", "8802");
    assert_eq!(
        words(&log, said, &group_texts(480)),
        Some(vec![
            "[15:00] 小林 (id=qq:20017) [msg=8802]: 在吗".to_string()
        ])
    );
}

#[test]
fn what_never_reaches_the_request_weighs_nothing() {
    let mut log = Log::new();
    let asleep = ambient(&mut log, "梦话", "8801", r#","asleep":true"#);
    let heard = ambient(&mut log, "早", "8802", "");
    assert_eq!(words(&log, asleep, &group_texts(480)), Some(Vec::new()));
    // 没有群聊近况的字的（私聊、以前造的会话），旁听的一句不进请求。
    assert_eq!(words(&log, heard, &texts()), Some(Vec::new()));
}

#[test]
fn a_terminal_message_stays_as_it_is_and_other_events_are_left_to_the_kernel() {
    let mut log = Log::new();
    let said = log.say("你好");
    log.start(said);
    let reply = log.reply(&format!("[{}]", text_json("在")));
    assert_eq!(words(&log, said, &texts()), Some(vec!["你好".to_string()]));
    assert_eq!(words(&log, reply, &texts()), None);
}
