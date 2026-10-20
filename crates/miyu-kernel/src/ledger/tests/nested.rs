//! 嵌套调用的几条规矩（施工 M-1，`kernel/history.md`「账本查的规矩」）：挂在调用下面的要那次调用还没有结果、带着回合；挂在
//! 任务下面的要那个后台命令还没报过结束、不带回合；同一个编号不收两次；父级结束了不再收。
//!
//! 底子：第 3 轮的回复里两个调用，第一个派了后台命令 `j1`（结果先到），第二个还在跑；`j2` 是子代理。

use super::*;

/// 一条了结的嵌套调用的 `body`。
fn nested(id: &str) -> String {
    format!(r#"{{"id":"{id}","name":"web_search","args":"{{}}","status":"ok"}}"#)
}

/// 底子：前 6 条。
fn base() -> Vec<Event> {
    vec![
        event(1, None, "session.created", CREATED),
        event(2, None, "message.user", SAID),
        event(3, Some(3), "turn.started", r#"{"trigger":2}"#),
        event(4, Some(3), "message.assistant", &reply(4, 3, 2, false)),
        event(
            5,
            Some(3),
            "tool.result",
            r#"{"call_id":"call_4_1","status":"ok","blocks":[],"effects":[{"kind":"job.started","job":"j1","what":"command","title":"t"},{"kind":"job.started","job":"j2","what":"agent","title":"t","session":"01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91"}]}"#,
        ),
        event(6, Some(3), "tool.nested", &nested("call_4_2#1")),
    ]
}

fn ledger() -> Ledger {
    let mut ledger = Ledger::default();
    for event in &base() {
        ledger.append(event).unwrap();
    }
    ledger
}

#[test]
fn a_nested_call_under_a_running_call_needs_its_turn() {
    let mut ledger = ledger();
    refused(
        &mut ledger,
        &event(7, None, "tool.nested", &nested("call_4_2#2")),
        "a nested call under a call happens only in its turn and needs turn",
    );
    // 带的回合不是那次调用所在的：回合的通则（`check_turn`）拦下，在跑的调用只在开着的那一轮里。
    refused(
        &mut ledger,
        &event(7, Some(2), "tool.nested", &nested("call_4_2#2")),
        "turn 2 is not the running turn",
    );
    refused(
        &mut ledger,
        &event(7, Some(3), "tool.nested", &nested("call_4_1#1")),
        "call_4_1 is not a call waiting for a result",
    );
    refused(
        &mut ledger,
        &event(7, Some(3), "tool.nested", &nested("call_4_9#1")),
        "call_4_9 is not a call waiting for a result",
    );
    refused(
        &mut ledger,
        &event(7, Some(3), "tool.nested", &nested("call_4_2#1")),
        "nested call call_4_2#1 is already settled",
    );
    // 另一个序号收；先到后到不管（并发的几个照了结的先后记）。
    ledger
        .append(&event(7, Some(3), "tool.nested", &nested("call_4_2#3")))
        .unwrap();
    ledger
        .append(&event(8, Some(3), "tool.nested", &nested("call_4_2#2")))
        .unwrap();
}

#[test]
fn nothing_settles_under_a_call_after_its_result() {
    let mut ledger = ledger();
    ledger
        .append(&event(7, Some(3), "tool.result", &result("call_4_2", "ok")))
        .unwrap();
    refused(
        &mut ledger,
        &event(8, Some(3), "tool.nested", &nested("call_4_2#2")),
        "call_4_2 is not a call waiting for a result",
    );
}

#[test]
fn a_nested_call_under_a_job_needs_a_running_command_and_no_turn() {
    let mut ledger = ledger();
    refused(
        &mut ledger,
        &event(7, Some(3), "tool.nested", &nested("j1#1")),
        "a nested call under a job has no turn",
    );
    refused(
        &mut ledger,
        &event(7, None, "tool.nested", &nested("j2#1")),
        "job j2 is not a running background command",
    );
    refused(
        &mut ledger,
        &event(7, None, "tool.nested", &nested("j9#1")),
        "job j9 is not a running background command",
    );
    ledger
        .append(&event(7, None, "tool.nested", &nested("j1#1")))
        .unwrap();
    refused(
        &mut ledger,
        &event(8, None, "tool.nested", &nested("j1#1")),
        "nested call j1#1 is already settled",
    );
    // 回合结束以后照样收：后台任务的嵌套调用在回合外落盘。
    ledger
        .append(&event(8, Some(3), "tool.result", &result("call_4_2", "ok")))
        .unwrap();
    ledger
        .append(&event(
            9,
            Some(3),
            "turn.ended",
            r#"{"reason":"completed"}"#,
        ))
        .unwrap();
    ledger
        .append(&event(10, None, "tool.nested", &nested("j1#2")))
        .unwrap();
    ledger
        .append(&event(
            11,
            None,
            "job.reported",
            r#"{"job":"j1","reason":"exited"}"#,
        ))
        .unwrap();
    refused(
        &mut ledger,
        &event(12, None, "tool.nested", &nested("j1#3")),
        "job j1 is not a running background command",
    );
}

/// 载入时给崩了的后台命令补的 `aborted` 报告也清掉它下面了结过的序号：`Settled` 不随日志长（miyu-core 审 M-1 时提的）。
#[test]
fn an_aborted_report_on_load_forgets_the_settled_numbers() {
    let mut ledger = ledger();
    let before = ledger.nested.clone();
    ledger
        .append(&event(7, None, "tool.nested", &nested("j1#1")))
        .unwrap();
    assert_ne!(ledger.nested, before);
    ledger
        .append(&event(
            8,
            None,
            "job.reported",
            r#"{"job":"j1","reason":"aborted"}"#,
        ))
        .unwrap();
    assert_eq!(ledger.nested, before);
}
