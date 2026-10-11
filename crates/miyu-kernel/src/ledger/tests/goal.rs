//! 长期目标的几条规矩（施工 X-6）：同一时刻至多一个没结束的；没有的改、停、接着跑、结束都拒；跑着的才停、停着的才接着跑；
//! 原文不能是空白；结束了能再设一个。

use super::*;

/// 合规的会话前 9 条（第一轮结束）再记一条长期目标的事件 `kind`、`body`，序号 10。
fn goal(seq: u64, kind: &str, body: &str) -> Event {
    event(seq, None, kind, body)
}

#[test]
fn a_goal_is_set_once_until_it_ends() {
    let mut ledger = after(9);
    refused(
        &mut ledger,
        &goal(10, "goal.set", r#"{"text":"  "}"#),
        "needs some text",
    );
    ledger
        .append(&goal(10, "goal.set", r#"{"text":"修绿","limit":3}"#))
        .unwrap();
    refused(
        &mut ledger,
        &goal(11, "goal.set", r#"{"text":"别的"}"#),
        "already set",
    );
    refused(
        &mut ledger,
        &goal(11, "goal.changed", r#"{"text":""}"#),
        "needs some text",
    );
    ledger
        .append(&goal(11, "goal.changed", r#"{"text":"修绿再写文档"}"#))
        .unwrap();
    ledger
        .append(&goal(12, "goal.ended", r#"{"outcome":"completed"}"#))
        .unwrap();
    ledger
        .append(&goal(13, "goal.set", r#"{"text":"下一个"}"#))
        .unwrap();
}

#[test]
fn without_a_goal_nothing_else_goes_and_pausing_follows_the_state() {
    let mut ledger = after(9);
    for (kind, body) in [
        ("goal.changed", r#"{"text":"x"}"#),
        ("goal.paused", "{}"),
        ("goal.resumed", "{}"),
        ("goal.ended", r#"{"outcome":"cleared"}"#),
    ] {
        refused(&mut ledger, &goal(10, kind, body), "needs a goal");
    }
    ledger
        .append(&goal(10, "goal.set", r#"{"text":"修绿"}"#))
        .unwrap();
    refused(&mut ledger, &goal(11, "goal.resumed", "{}"), "not paused");
    ledger.append(&goal(11, "goal.paused", "{}")).unwrap();
    refused(
        &mut ledger,
        &goal(12, "goal.paused", "{}"),
        "already paused",
    );
    refused(
        &mut ledger,
        &goal(12, "goal.set", r#"{"text":"别的"}"#),
        "already set",
    );
    ledger.append(&goal(12, "goal.resumed", "{}")).unwrap();
    ledger.append(&goal(13, "goal.paused", "{}")).unwrap();
    ledger.append(&goal(14, "goal.resumed", "{}")).unwrap();
    // 不认识的结局也算结束了。
    ledger
        .append(&goal(15, "goal.ended", r#"{"outcome":"abandoned"}"#))
        .unwrap();
    refused(&mut ledger, &goal(16, "goal.paused", "{}"), "needs a goal");
}
