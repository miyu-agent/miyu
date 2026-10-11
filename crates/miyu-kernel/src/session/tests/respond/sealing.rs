//! 照记下的几条开的一轮在支线里算新的一轮（施工 C-8 下，`seal.rs`）：上一轮跑的时候旁听记下的一条，上一轮结束以后桥照它开
//! 一轮，触发落在上一轮里，可它是照记下的几条开的（`triggers` 不空），不是接着开的。

use std::num::NonZeroU32;

use super::*;
use crate::event::Forked;
use crate::testkit::CHILD_SESSION;

/// 主会话说完一轮，从它的最后一条分叉，跑满 `after` 轮封存：交回子会话，第一条落了盘。
fn branch(after: u32) -> Logged {
    let mut parent = Logged::new();
    let seen = parent.ask(1, "hi");
    parent.say(seen, "好");
    let upto = parent.last();
    let mut created: SessionCreated = serde_json::from_str(CREATED).unwrap();
    created.parent = Some(session_id());
    created.depth = Some(1);
    created.forked = Some(Forked {
        from: session_id(),
        upto: seq(upto),
        seal_after: NonZeroU32::new(after),
    });
    let child = SessionId::parse(CHILD_SESSION).unwrap();
    let env = environment("~/src/miyu");
    let (session, actions) = Session::fork(
        child,
        &parent.log,
        id(100),
        alice(),
        at(30),
        created,
        policy(),
        env,
    )
    .unwrap();
    let mut logged = Logged {
        session,
        log: appended_events(&actions),
    };
    logged.handle(stored(upto + 1));
    logged
}

#[test]
fn a_turn_opened_on_what_was_overheard_during_the_last_one_is_a_new_round() {
    let mut child = branch(2);
    let seen = child.ask(2, "一");
    child.handle(overheard(3));
    let heard = child.last();
    child.handle(stored(heard));
    child.say(seen, "嗯");
    assert!(!child.session.sealed(), "还差一轮");
    child.handle(respond(4, &[heard]));
    child.handle(stored(child.last()));
    let turn = child
        .log
        .iter()
        .rev()
        .find(|event| matches!(event.body, Body::TurnStarted(_)))
        .map(|event| TurnId::new(event.seq))
        .unwrap();
    let actions = child.handle(hooks_done(turn, Vec::new()));
    let (seen, _) = calls(&actions).remove(0);
    child.say(seen.get(), "好");
    assert!(child.session.sealed(), "照记下的开的是新的一轮");
}
