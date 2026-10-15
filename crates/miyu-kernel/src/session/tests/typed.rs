//! 斜杠命令模板展开成的一句（施工 X-14 上，`docs/blueprint/slash.md`「模板」第 4、6 条）：照 `session.send` 发，`message.user`
//! 带上人敲的那一行；重做照原样重发的还是那条命令，换了字的不再是。

use super::load::Logged;
use super::*;
use crate::event::{MessageUser, TypedCommand};

/// 展开成 `expanded` 的那段。
fn text(expanded: &str) -> Vec<Block> {
    vec![Block::Text(Text {
        text: expanded.to_string(),
    })]
}

/// `/review a.rs`。
fn review() -> TypedCommand {
    TypedCommand {
        name: "review".to_string(),
        text: "/review a.rs".to_string(),
    }
}

/// `n` 号命令：alice 敲的 `/review a.rs`，展开成 `expanded`。
fn typed(n: u64, expanded: &str) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(n),
        command: Command::Typed {
            blocks: text(expanded),
            command: review(),
        },
    })
}

/// `n` 号命令：重做最后一轮，开这一轮的那一句换成 `words`（没有的原样）。
fn redo(n: u64, words: Option<&str>) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(56),
        command: Command::Redo {
            text: words.map(text),
            attachments: None,
        },
    })
}

#[test]
fn a_typed_command_is_sent_like_a_message_and_keeps_the_line() {
    let mut session = session();
    let actions = session.handle(typed(1, "Review a.rs carefully."));
    let events = appended_events(&actions);
    let mut plain = super::session();
    let sent = appended_events(&plain.handle(send(1, "Review a.rs carefully.")));
    assert_eq!(
        events
            .iter()
            .map(|event| event.body.kind())
            .collect::<Vec<_>>(),
        sent.iter()
            .map(|event| event.body.kind())
            .collect::<Vec<_>>(),
        "闲着照 session.send 开一轮"
    );
    assert_eq!(
        events[0].body,
        Body::MessageUser(MessageUser {
            blocks: text("Review a.rs carefully."),
            venue: None,
            command: Some(review()),
        })
    );
    let Body::MessageUser(message) = &events[0].body else {
        unreachable!()
    };
    assert_eq!(message.typed(), Some("/review a.rs"));
}

#[test]
fn while_a_turn_runs_it_queues_without_cutting_in() {
    let mut logged = Logged::new();
    logged.ask(1, "hi");
    let mut plain = Logged::new();
    plain.ask(1, "hi");
    let kinds = |actions: &[Action]| -> Vec<(String, bool)> {
        appended_events(actions)
            .iter()
            .map(|event| (event.body.kind().to_string(), event.turn.is_some()))
            .collect()
    };
    assert_eq!(
        kinds(&logged.handle(typed(2, "Review."))),
        kinds(&plain.handle(send(2, "Review."))),
        "排进这一轮，不急着插话"
    );
}

/// 一轮由 `/review a.rs` 开、说完了，全落了盘。
fn typed_turn() -> Logged {
    let mut logged = Logged::new();
    logged.handle(typed(1, "Review a.rs carefully."));
    logged.handle(stored(logged.last()));
    let turn = logged
        .log
        .iter()
        .rev()
        .find(|event| matches!(event.body, Body::TurnStarted(_)))
        .map(|event| TurnId::new(event.seq))
        .unwrap();
    let actions = logged.handle(hooks_done(turn, Vec::new()));
    let seen = calls(&actions).remove(0).0.get();
    logged.say(seen, "好");
    logged
}

/// 重做重发的那一句。
fn resent(actions: &[Action]) -> MessageUser {
    appended_events(actions)
        .into_iter()
        .find_map(|event| match event.body {
            Body::MessageUser(message) => Some(message),
            _ => None,
        })
        .expect("重发了一句")
}

#[test]
fn redoing_as_it_was_keeps_the_command() {
    let mut logged = typed_turn();
    let message = resent(&logged.handle(redo(2, None)));
    assert_eq!(message.command, Some(review()));
    assert_eq!(message.blocks, text("Review a.rs carefully."), "不重读模板");
}

#[test]
fn redoing_with_new_words_is_no_longer_the_command() {
    let mut logged = typed_turn();
    let message = resent(&logged.handle(redo(2, Some("换个说法"))));
    assert_eq!(message.command, None);
    assert_eq!(message.blocks, text("换个说法"));
}
