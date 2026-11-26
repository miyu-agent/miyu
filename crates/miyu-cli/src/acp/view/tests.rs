use serde_json::{Value, json};

use super::{Action, Pace};
use crate::acp::session::{Prompt, Session, Stop, Waiting};

/// 一条条目，照视图流里的写法。
fn entry(id: &str, kind: &str, fields: Value) -> Value {
    let mut base = json!({"id": id, "kind": kind, "at": "2026-11-26T00:00:00.000Z"});
    if let (Some(base), Some(fields)) = (base.as_object_mut(), fields.as_object()) {
        base.extend(fields.clone());
    }
    base
}

/// 一条 `session/update` 的 `update`。
fn updates(actions: &[Action]) -> Vec<&Value> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::Update(update) => Some(update),
            _ => None,
        })
        .collect()
}

#[test]
fn reply_text_goes_out_once_whether_added_appended_or_updated() {
    let mut session = Session::new("/work");
    let add = json!({"entry": entry("b5.0", "reply", json!({"text": "你", "open": true, "persona": null}))});
    let first = session.push("view.add", &add);
    assert_eq!(updates(&first)[0]["content"]["text"], "你");
    let more = session.push("view.append", &json!({"id": "b5.0", "text": "好"}));
    assert_eq!(updates(&more)[0]["content"]["text"], "好");
    assert_eq!(updates(&more)[0]["sessionUpdate"], "agent_message_chunk");
    // 收尾的整份：报过的不再报，多出来的报那一截。
    let update =
        json!({"entry": entry("b5.0", "reply", json!({"text": "你好。", "persona": null}))});
    let last = session.push("view.update", &update);
    assert_eq!(updates(&last).len(), 1);
    assert_eq!(updates(&last)[0]["content"]["text"], "。");
    let again = session.push("view.update", &update);
    assert!(again.is_empty(), "{again:?}");
}

#[test]
fn thoughts_go_out_as_thought_chunks() {
    let mut session = Session::new("/work");
    let raw = entry("b5.1", "thought", json!({"group": "g1", "text": "想"}));
    let actions = session.entry(&raw, Pace::Live);
    assert_eq!(updates(&actions)[0]["sessionUpdate"], "agent_thought_chunk");
}

#[test]
fn the_editors_own_words_are_not_echoed_but_others_are() {
    let mut session = Session::new("/work");
    session.own.insert("m4".to_string());
    let own = entry("m4", "user", json!({"text": "我说的"}));
    assert!(session.entry(&own, Pace::Live).is_empty());
    let other = entry("m9", "user", json!({"text": "终端里说的"}));
    let actions = session.entry(&other, Pace::Live);
    assert_eq!(updates(&actions)[0]["sessionUpdate"], "user_message_chunk");
    assert!(session.entry(&other, Pace::Live).is_empty(), "只报一次");
    // 重放时编辑器没画过，自己的也报；只记下的一个都不报。
    let mut replayed = Session::new("/work");
    assert_eq!(updates(&replayed.entry(&own, Pace::Replay)).len(), 1);
    let mut quiet = Session::new("/work");
    assert!(quiet.entry(&other, Pace::Quiet).is_empty());
}

#[test]
fn hidden_entries_are_not_reported() {
    let mut session = Session::new("/work");
    let hidden = entry("m9", "user", json!({"text": "撤掉的", "hidden": true}));
    assert!(session.entry(&hidden, Pace::Replay).is_empty());
}

#[test]
fn the_prompt_ends_with_the_turn_its_message_started() {
    let mut session = Session::new("/work");
    session.own.insert("m4".to_string());
    session.prompt = Some(Prompt {
        id: json!(7),
        waiting: Waiting::Message("m4".to_string()),
        cancelled: false,
    });
    let end = |turn: u64| {
        entry(
            &format!("e{turn}"),
            "end",
            json!({"reason": "completed", "took_ms": 5, "turn": turn}),
        )
    };
    // 还排着、别的一轮结束了：不回。
    session.entry(
        &entry("m4", "user", json!({"text": "在吗", "queued": true})),
        Pace::Live,
    );
    assert!(session.entry(&end(2), Pace::Live).is_empty());
    // 听到了、属于第 5 轮：第 5 轮结束时回。
    session.entry(
        &entry("m4", "user", json!({"text": "在吗", "turn": 5})),
        Pace::Live,
    );
    let actions = session.entry(&end(5), Pace::Live);
    assert_eq!(
        actions,
        [Action::Answer {
            id: json!(7),
            stop: Stop::Reason("end_turn")
        }]
    );
    assert!(session.prompt.is_none());
}

#[test]
fn a_running_tool_is_not_remembered_quietly() {
    // 只记下的会话里还在跑的一步：之后来的变化照第一次见报 `tool_call`，编辑器才认得这个编号。
    let mut session = Session::new("/work");
    let running = entry(
        "b3.1",
        "tool",
        json!({"group": "g1", "name": "shell", "title": {"name": "运行"},
        "state": "running", "args": "{}"}),
    );
    assert!(session.entry(&running, Pace::Quiet).is_empty());
    let actions = session.entry(&running, Pace::Live);
    assert_eq!(updates(&actions)[0]["sessionUpdate"], "tool_call");
}

#[test]
fn a_finished_edit_asks_for_its_diff() {
    let mut session = Session::new("/work");
    let raw = |state: &str, files: Value| {
        entry(
            "b3.1",
            "tool",
            json!({"group": "g1", "call": "call_3_1", "name": "edit",
            "title": {"name": "编辑"}, "state": state, "args": "{}", "files": files}),
        )
    };
    session.entry(&raw("running", json!([])), Pace::Live);
    let done = raw("ok", json!([{"path": "/w/a.md", "action": "changed"}]));
    let actions = session.entry(&done, Pace::Live);
    assert!(
        matches!(&actions[..], [Action::Detailed { call, .. }] if call == "call_3_1"),
        "{actions:?}"
    );
    // 扔进回收站的不要对比。
    let mut trashed = Session::new("/work");
    let gone = raw("ok", json!([{"path": "/w/a.md", "action": "trashed"}]));
    assert!(matches!(
        &trashed.entry(&gone, Pace::Live)[..],
        [Action::Update(_)]
    ));
}

#[test]
fn every_chunk_names_the_entry_it_belongs_to() {
    // 编号变了编辑器另起一块：上一轮打断了的思考和下一轮的思考不连成一句（真 Zed 里看到过）。
    let mut session = Session::new("/work");
    let first = entry(
        "b5.0",
        "thought",
        json!({"group": "g1", "text": "想", "turn": 3}),
    );
    let second = entry(
        "b9.0",
        "thought",
        json!({"group": "g2", "text": "再想", "turn": 3}),
    );
    session.introduced.insert("3".to_string());
    let a = session.entry(&first, Pace::Live);
    let b = session.entry(&second, Pace::Live);
    assert_eq!(updates(&a)[0]["messageId"], "b5.0");
    assert_eq!(updates(&b)[0]["messageId"], "b9.0");
    let more = session.push("view.append", &json!({"id": "b9.0", "text": "想"}));
    assert_eq!(updates(&more)[0]["messageId"], "b9.0");
}

#[test]
fn a_turn_nobody_started_gets_an_opening_line() {
    let mut session = Session::new("/work");
    // 人说的话开的一轮：有开头，不另报。
    session.entry(
        &entry("m4", "user", json!({"text": "在吗", "turn": 5})),
        Pace::Live,
    );
    let reply = entry(
        "b6.0",
        "reply",
        json!({"text": "在。", "persona": null, "turn": 5}),
    );
    assert!(
        !session
            .entry(&reply, Pace::Live)
            .iter()
            .any(|action| matches!(action, Action::Woken { .. }))
    );
    // 后台任务报回来叫醒的一轮：先报一句开头，说是哪一件；同一轮只报一次。
    let job = entry(
        "e20",
        "notice",
        json!({"what": "job", "job": "j1", "job_kind": "shell", "title": "跑测试", "mark": "done"}),
    );
    session.entry(&job, Pace::Live);
    let woke = entry(
        "b22.0",
        "thought",
        json!({"group": "g9", "text": "测试过了", "turn": 21}),
    );
    let actions = session.entry(&woke, Pace::Live);
    assert_eq!(
        actions[0],
        Action::Woken {
            turn: "21".to_string(),
            job: Some(("跑测试".to_string(), "done".to_string()))
        }
    );
    let again = entry(
        "b22.1",
        "reply",
        json!({"text": "好了", "persona": null, "turn": 21}),
    );
    assert!(
        !session
            .entry(&again, Pace::Live)
            .iter()
            .any(|action| matches!(action, Action::Woken { .. }))
    );
    // 编辑器在等的那一句还没听到：这一轮当是它开的。
    session.prompt = Some(Prompt {
        id: json!(1),
        waiting: Waiting::Message("m30".to_string()),
        cancelled: false,
    });
    let ours = entry(
        "b31.0",
        "reply",
        json!({"text": "嗯", "persona": null, "turn": 31}),
    );
    assert!(
        !session
            .entry(&ours, Pace::Live)
            .iter()
            .any(|action| matches!(action, Action::Woken { .. }))
    );
    // 只记下的不报。
    let mut quiet = Session::new("/work");
    assert!(quiet.entry(&woke, Pace::Quiet).is_empty());
}
