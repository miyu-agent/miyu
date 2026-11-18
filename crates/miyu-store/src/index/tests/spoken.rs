//! 人最后一次发话（施工 SX-1，`store/index.md`「一行记什么」的 `last_spoken`）：只认 `by` 是人的 `message.user`，主人、通讯平台
//! 上的人都算；定时、循环、内核续跑、别的会话发来的、模型的回复都不动它，只动 `last_active`。写进表里读得回来；
//! 列会话照日志补的那一行写回去也带着它。

use super::*;

/// 第 `seq` 条，`by` 照写，时刻是 07:00 过 `seq` 秒。
fn spoken(seq: u64, by: &str) -> Event {
    event(&format!(
        r#"{{"seq":{seq},"at":"2026-09-25T07:00:{seq:02}.000Z","kind":"message.user","by":{by},"cause":"c{seq}","body":{{"blocks":[{{"type":"text","text":"在吗"}}]}}}}"#
    ))
}

fn replied(seq: u64) -> Event {
    event(&format!(
        r#"{{"seq":{seq},"at":"2026-09-25T07:00:{seq:02}.000Z","kind":"message.assistant","turn":2,"by":{{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}},"body":{{"blocks":[{{"type":"text","text":"在。"}}],"seen":{seq}}}}}"#
    ))
}

const PERSON: &str = r#"{"kind":"person"}"#;

#[test]
fn only_what_a_person_says_moves_it() {
    let mut listed = Row::new(id(), &created()).unwrap();
    assert_eq!(listed.last_spoken, None, "还没人说过话");
    listed.see(&spoken(2, PERSON));
    let first = listed.last_spoken.expect("人说了一句");
    assert_eq!(first.to_string(), "2026-09-25T07:00:02.000Z");
    let others = [
        replied(3),
        spoken(4, r#"{"kind":"schedule","schedule":"t1"}"#),
        spoken(5, r#"{"kind":"schedule","schedule":"t2","loop":"fixed"}"#),
        spoken(6, r#"{"kind":"schedule","schedule":"t3","loop":"dynamic"}"#),
        spoken(8, r#"{"kind":"kernel"}"#),
        spoken(
            9,
            r#"{"kind":"session","id":"01a0f234-0a1b-7c2d-8e3f-4a5b6c7d8e9f"}"#,
        ),
    ];
    for other in &others {
        listed.see(other);
        assert_eq!(listed.last_spoken, Some(first), "{other:?} 不是人说的");
        assert_eq!(listed.last_active, other.at, "动静照样往后走");
    }
    listed.see(&spoken(
        10,
        r#"{"kind":"external","venue":"qq:group:5","id":"qq:2"}"#,
    ));
    assert_eq!(
        listed.last_spoken.map(|at| at.to_string()).as_deref(),
        Some("2026-09-25T07:00:10.000Z"),
        "群里的人说的也算"
    );
}

#[test]
fn it_survives_the_table_and_a_caught_up_row() {
    let scratch = Scratch::new();
    let (index, _) = open(&scratch);
    let mut listed = Row::new(id(), &created()).unwrap();
    listed.mark = mark(100, 2);
    assert!(index.put(&listed, None).unwrap());
    assert_eq!(row(&index).unwrap().last_spoken, None, "没有的存成空的");
    // 列会话照日志补上多出来的一截，只换照到的没变过的那一行：补上的那一格要跟着写回去。
    let was = listed.mark;
    listed.see(&spoken(2, PERSON));
    listed.mark = mark(300, 3);
    assert!(index.put(&listed, Some(&was)).unwrap());
    assert_eq!(row(&index).unwrap(), listed, "写回去读得回来");
}
