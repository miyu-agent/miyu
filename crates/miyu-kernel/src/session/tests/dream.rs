//! 整理会话跑完一趟（施工 DR-1，`session/dream.rs`）：记一条 `dream.ended`，不带回合编号（回合进行中也不带），落了盘才回应；
//! 没叫停的不写 `stopped`；载入时照常读回。

use super::load::{Logged, load};
use super::*;
use crate::event::DreamEnded;

/// 编号是 `n` 的命令：交这一趟的结果。
fn end_dream(n: u64, ended: DreamEnded) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(n % 60),
        command: Command::EndDream(ended),
    })
}

const RESULT: DreamEnded = DreamEnded {
    given: 3,
    revised: 1,
    retired: 2,
    summary: true,
    stopped: false,
};

#[test]
fn the_result_is_recorded_outside_the_turn_and_answered_once_stored() {
    let mut logged = Logged::new();
    logged.ask(1, "开工");
    let actions = logged.handle(end_dream(2, RESULT));
    let appended = appended_events(&actions);
    let [event] = appended.as_slice() else {
        panic!("记一条：{actions:?}");
    };
    assert!(matches!(&event.body, Body::DreamEnded(ended) if *ended == RESULT));
    assert_eq!(event.turn, None, "回合进行中也不带回合编号");
    assert_eq!(event.cause, Some(id(2)));
    assert!(replies(&actions).is_empty(), "还没落盘不回应：{actions:?}");
    let actions = logged.handle(stored(event.seq.get()));
    assert!(
        replies(&actions).iter().any(|action| matches!(
            action,
            Action::Reply { id: replied, outcome: Outcome::Accepted { events } }
                if *replied == id(2) && *events == vec![event.seq]
        )),
        "落了盘回应：{actions:?}"
    );
    let (session, _) = load(logged.log.clone());
    drop(session);
}

#[test]
fn stopped_is_written_only_when_true() {
    let json = serde_json::to_value(RESULT).expect("写得出");
    assert_eq!(
        json,
        serde_json::json!({"given": 3, "revised": 1, "retired": 2, "summary": true})
    );
    let halted = DreamEnded {
        stopped: true,
        ..RESULT
    };
    let json = serde_json::to_value(halted).expect("写得出");
    assert_eq!(json["stopped"], true);
    let back: DreamEnded = serde_json::from_value(json).expect("读得回");
    assert_eq!(back, halted);
}
