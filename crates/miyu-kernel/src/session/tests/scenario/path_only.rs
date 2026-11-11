//! 场景：只有路径的附件（施工 AT-1，`docs/blueprint/kernel/blocks.md` 第 14 条）。内核把它当一块普通的附件：记进
//! `message.user`，只有它也是一句话；重做照带、照换，撤销恢复以后还在；压缩把带着它的那一轮收进摘要以后照常往下走；
//! 本地估算照这一块规范的 JSON 数。

use super::compaction::{compacting, line};
use super::*;
use crate::block::{PathOnly, Text};
use crate::estimate::{self, Flat};
use crate::id::{FileName, SourcePath};

/// 一段话写成的一块文字。
fn text(words: &str) -> Block {
    Block::Text(Text {
        text: words.to_string(),
    })
}

/// 一个只有路径的附件；`size` 是头写的大小。
fn attached(name: &str, size: Option<u64>) -> Block {
    Block::Path(PathOnly {
        name: FileName::parse(name).unwrap(),
        path: SourcePath::parse(&format!("/data/{name}")).unwrap(),
        size,
    })
}

/// 日志里人说的话，照先后。
fn said(s: &Stage) -> Vec<Vec<Block>> {
    s.log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::MessageUser(message) => Some(message.blocks.clone()),
            _ => None,
        })
        .collect()
}

/// 最后一轮的编号。
fn last_turn(s: &Stage) -> TurnId {
    let started = s
        .log()
        .iter()
        .rfind(|event| matches!(event.body, Body::TurnStarted(_)))
        .expect("开过一轮");
    TurnId::new(started.seq)
}

#[test]
fn it_rides_along_with_redo_and_undo_like_any_attachment() {
    let mut s = stage();
    s.model([
        Line::says("看到了"),
        Line::says("又看了"),
        Line::says("换了一个"),
        Line::says("只有它"),
    ]);
    let first = vec![text("看看"), attached("disk.iso", Some(31_457_280))];
    let id = s.send(first.clone());
    assert!(
        matches!(s.outcome(&id), Some(Outcome::Accepted { .. })),
        "{:?}",
        s.outcome(&id)
    );
    assert_eq!(said(&s), std::slice::from_ref(&first));
    // 撤销、恢复：那一句连着这一块回来，日志里还是那一条。
    let turn = last_turn(&s);
    s.revert(turn);
    let restored = s.unrevert();
    assert!(
        matches!(s.outcome(&restored), Some(Outcome::Accepted { .. })),
        "{:?}",
        s.outcome(&restored)
    );
    // 只换话：这一块照带，字在前、附件在后。
    s.redo(Some(vec![text("再看看")]));
    let resent = vec![text("再看看"), attached("disk.iso", Some(31_457_280))];
    assert_eq!(said(&s).last(), Some(&resent));
    // 换附件：换成另一个没写大小的。
    s.redo_with(None, Some(vec![attached("b.bin", None)]));
    let swapped = vec![text("再看看"), attached("b.bin", None)];
    assert_eq!(said(&s).last(), Some(&swapped));
    // 字换成空的：只剩它，也是一句话。
    let id = s.redo_with(Some(Vec::new()), None);
    assert!(
        matches!(s.outcome(&id), Some(Outcome::Accepted { .. })),
        "{:?}",
        s.outcome(&id)
    );
    assert_eq!(said(&s).last(), Some(&vec![attached("b.bin", None)]));
    // 每一条事件写出去再读回来一字不差。
    for event in s.log() {
        let written = serde_json::to_string(event).unwrap();
        let read: Event = serde_json::from_str(&written).unwrap();
        assert_eq!(serde_json::to_string(&read).unwrap(), written);
    }
}

#[test]
fn a_turn_with_it_is_summarized_and_the_session_goes_on() {
    let mut s = compacting(None);
    s.model([Line::says("好。")]);
    s.send(vec![text("看看"), attached("disk.iso", Some(7))]);
    line(&mut s, 100);
    s.model([Line::says("S1"), Line::says("嗯。")]);
    s.say("再说一句");
    let kinds: Vec<String> = story(&s);
    assert!(
        kinds.iter().any(|line| line.contains("context.compacted")),
        "压了：{kinds:?}"
    );
    assert!(
        kinds
            .last()
            .is_some_and(|line| line.contains("turn.ended:completed")),
        "压完照常说完：{kinds:?}"
    );
    // 摘要请求里列着带这一块的那一句：它在检查点前面，收进了摘要。
    assert_eq!(said(&s).len(), 2);
}

/// 本地估算（`compaction.md` 第二条第 2 条）：这一块照它规范的 JSON 的字节数算进字节里，不照文件的固定数。
#[test]
fn it_is_estimated_by_its_json_bytes() {
    let block = attached("disk.iso", Some(7));
    let json = serde_json::to_string(&block).unwrap();
    assert_eq!(
        json,
        r#"{"type":"path","name":"disk.iso","path":"/data/disk.iso","size":7}"#
    );
    let price = Flat {
        image: 2000,
        file: 2000,
    };
    let message = Message::User {
        blocks: vec![text("abcd"), block],
    };
    let bytes = u64::try_from(4 + json.len()).unwrap();
    assert_eq!(estimate::message(&message, &price), bytes.div_ceil(4));
}
