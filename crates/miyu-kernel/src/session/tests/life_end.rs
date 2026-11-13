//! 人生结束（施工 LF-4 上，`docs/blueprint/life.md`「结束」）：`life.ended` 不带回合，只记一次；闲着的同一批封存，在跑一轮的
//! 这一轮结束时封存；记了以后改对话的命令拒 `session_sealed`，排着的不再开轮。

use super::compact::rejected_reply;
use super::executor::*;
use super::*;
use crate::event::LifeEnded;
use crate::session::Appended;

/// 编号是 `n` 的命令：人记 `life.ended`。
fn end(n: u64) -> Input {
    Input::Command(Received {
        id: id(n),
        by: person(),
        at: at(n % 60),
        command: Command::Append {
            event: Appended::LifeEnded(LifeEnded { last_words: None }),
        },
    })
}

#[test]
fn an_idle_life_is_sealed_with_its_end_and_ends_once() {
    let mut session = session();
    let actions = session.handle(end(1));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[2, 3]), "{events:?}");
    assert_eq!(events[0].body.kind(), "life.ended");
    assert_eq!(events[0].turn, None);
    assert_eq!(events[1].body.kind(), "session.sealed");
    session.handle(stored(3));
    assert!(session.life_ended() && session.sealed());
    let actions = session.handle(send(2, "还在吗"));
    assert!(
        actions.contains(&rejected_reply(2, Reason::Sealed)),
        "{actions:?}"
    );
    let actions = session.handle(end(3));
    assert!(
        actions.contains(&rejected_reply(3, Reason::Sealed)),
        "只记一次：{actions:?}"
    );
}

#[test]
fn a_running_turn_ends_first_then_the_session_is_sealed() {
    let mut session = asking();
    let actions = session.handle(end(2));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[6]), "在跑一轮的不当场封存");
    assert_eq!(events[0].body.kind(), "life.ended");
    session.handle(stored(6));
    assert!(session.life_ended() && !session.sealed());
    let actions = session.handle(send(3, "还在吗"));
    assert!(
        actions.contains(&rejected_reply(3, Reason::Sealed)),
        "结束了就不收话：{actions:?}"
    );
    let actions = answer(&mut session, 5, "再见。");
    let events = appended_events(&actions);
    let kinds: Vec<&str> = events.iter().map(|event| event.body.kind()).collect();
    let ended = kinds.iter().position(|kind| *kind == "turn.ended");
    let sealed = kinds.iter().position(|kind| *kind == "session.sealed");
    assert!(
        ended.is_some() && sealed > ended,
        "这一轮说完才封存：{kinds:?}"
    );
    assert!(!kinds.contains(&"turn.started"), "不再开轮：{kinds:?}");
}
