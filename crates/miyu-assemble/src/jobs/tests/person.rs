//! 子会话里人直接说的话（施工 PM-1）：子会话自己那一段里的包一层 `<user-message>`，和父会话写的交代分开；父会话的交代、
//! 主会话里人说的话、分叉出来的会话的底子里人对父会话说的话照旧原样（前缀和父会话一样）；旧快照没有标签的照旧原样。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Event;
use miyu_kernel::history::History;
use miyu_kernel::id::Seq;
use miyu_kernel::origin::{By, Person, Session};

use super::*;

/// 父会话的编号。
const PARENT: &str = "01a19a00-0000-7000-8000-000000000001";

fn block(text: &str) -> Vec<Block> {
    vec![Block::Text(Text {
        text: text.to_string(),
    })]
}

fn person() -> By {
    By::Person(Person::default())
}

fn parent() -> By {
    By::Session(Session {
        id: SessionId::parse(PARENT).unwrap(),
    })
}

fn at(n: u64) -> Seq {
    Seq::new(n).unwrap()
}

#[test]
fn in_a_child_session_the_persons_words_are_tagged() {
    let texts = shipped();
    let child = Log::child(PARENT);
    assert_eq!(
        message(
            child.history(),
            (at(5), &person()),
            block("先别改测试"),
            Some(&texts)
        ),
        block("<user-message>\n先别改测试\n</user-message>\n")
    );
    assert_eq!(
        message(
            child.history(),
            (at(2), &parent()),
            block("查 A"),
            Some(&texts)
        ),
        block("查 A"),
        "父会话写的交代照旧原样"
    );
    let main = Log::new();
    assert_eq!(
        message(
            main.history(),
            (at(5), &person()),
            block("先别改测试"),
            Some(&texts)
        ),
        block("先别改测试"),
        "主会话里人说的照旧原样：字节不变"
    );
    let old = JobTexts {
        user_message_open: String::new(),
        user_message_close: String::new(),
        ..shipped()
    };
    assert_eq!(
        message(
            child.history(),
            (at(5), &person()),
            block("先别改测试"),
            Some(&old)
        ),
        block("先别改测试"),
        "旧快照没有标签：照旧原样"
    );
}

/// 分叉出来的会话：底子里人对父会话说的话照父会话的原样（前缀和父会话一样，缓存才吃得到），分叉点以后人切进来说的才包。
/// 有效历史是父会话的底子接上分叉出来的会话自己的 `session.created`（两条 `session.created`），照内核载入分叉会话那样直接拼。
#[test]
fn a_forks_base_keeps_the_parents_words_as_they_were() {
    let texts = shipped();
    let policy = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let created = r#""venue":"local","permission":{"level":"workspace","read_only":false}"#;
    let line = |seq: u64, kind: &str, by: &str, body: &str| {
        Event::from_line(&format!(
            r#"{{"seq":{seq},"at":"2026-09-25T07:00:00.000Z","kind":"{kind}","by":{by},"body":{body}}}"#
        ))
        .unwrap()
    };
    let mut history = History::default();
    history.append(line(
        1,
        "session.created",
        KERNEL,
        &format!(r#"{{{created},"policy":"{policy}"}}"#),
    ));
    history.append(line(
        2,
        "message.user",
        r#"{"kind":"person"}"#,
        r#"{"blocks":[]}"#,
    ));
    history.append(line(
        3,
        "session.created",
        KERNEL,
        &format!(r#"{{{created},"policy":"{policy}","parent":"{PARENT}","depth":1}}"#),
    ));
    assert_eq!(
        message(
            &history,
            (at(2), &person()),
            block("帮我看看 CI"),
            Some(&texts)
        ),
        block("帮我看看 CI"),
        "底子里的照原样"
    );
    assert_eq!(
        message(&history, (at(4), &person()), block("先别动"), Some(&texts)),
        block("<user-message>\n先别动\n</user-message>\n"),
        "分叉点以后人说的包"
    );
}
