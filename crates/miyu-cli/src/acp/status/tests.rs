use serde_json::json;

use crate::acp::session::Session;
use crate::acp::view::Action;

#[test]
fn todos_become_a_plan_and_repeat_only_when_they_change() {
    let mut session = Session::new("/work");
    // 从来没有待办的不报空的。
    assert!(session.status(&json!({"todos": []})).is_empty());
    let status = json!({"todos": [{"content": "读代码", "status": "in_progress"}, {"content": "改", "status": "pending"}]});
    let actions = session.status(&status);
    assert_eq!(
        actions,
        [Action::Update(json!({"sessionUpdate": "plan", "entries": [
            {"content": "读代码", "priority": "medium", "status": "in_progress"},
            {"content": "改", "priority": "medium", "status": "pending"},
        ]}))]
    );
    assert!(session.status(&status).is_empty());
    // 清空了报一份空的。
    let actions = session.status(&json!({"todos": []}));
    assert_eq!(
        actions,
        [Action::Update(
            json!({"sessionUpdate": "plan", "entries": []})
        )]
    );
}

#[test]
fn usage_needs_both_numbers_and_only_precise_money() {
    let mut session = Session::new("/work");
    assert!(session.status(&json!({"context": {"used": 10}})).is_empty());
    let status = json!({"context": {"used": 10, "window": 100},
        "usage": {"unpriced": 0, "amounts": [{"currency": "USD", "amount": 0.5}]}});
    let actions = session.status(&status);
    assert_eq!(
        actions,
        [Action::Update(
            json!({"sessionUpdate": "usage_update", "used": 10, "size": 100,
            "cost": {"amount": 0.5, "currency": "USD"}})
        )]
    );
    // 几种币的照换算好的；有换算不了的、有没算出钱的不报金额。
    let mixed = json!({"context": {"used": 11, "window": 100}, "usage": {"unpriced": 0,
        "amounts": [{"currency": "USD", "amount": 0.5}, {"currency": "CNY", "amount": 1}],
        "converted": {"currency": "CNY", "amount": 4.6}}});
    let actions = session.status(&mixed);
    assert_eq!(
        actions,
        [Action::Update(
            json!({"sessionUpdate": "usage_update", "used": 11, "size": 100,
            "cost": {"amount": 4.6, "currency": "CNY"}})
        )]
    );
    let unpriced = json!({"context": {"used": 12, "window": 100}, "usage": {"unpriced": 1,
        "amounts": [{"currency": "USD", "amount": 0.5}]}});
    let actions = session.status(&unpriced);
    assert_eq!(
        actions,
        [Action::Update(
            json!({"sessionUpdate": "usage_update", "used": 12, "size": 100})
        )]
    );
}

#[test]
fn a_permission_or_model_change_asks_for_the_options() {
    let mut session = Session::new("/work");
    let status = json!({"permission": {"level": "full", "read_only": true},
        "model": {"ref": "deepseek/flash", "effort": {"level": "high", "from": "system"}}});
    assert_eq!(session.status(&status), [Action::Config]);
    assert_eq!(session.current.mode, "read-only");
    assert_eq!(session.current.model.as_deref(), Some("deepseek/flash"));
    assert_eq!(session.current.effort.as_deref(), Some("high"));
    assert!(session.status(&status).is_empty());
}

#[test]
fn waiting_is_handed_on_and_settled_ones_are_forgotten() {
    let mut session = Session::new("/work");
    let waiting = json!({"waiting": [{"what": "approve", "entry": "b3.1", "call": "call_3_1"}]});
    let actions = session.status(&waiting);
    assert!(matches!(&actions[..], [Action::Waiting(list)] if list.len() == 1));
    session.asked.insert("call_3_1".to_string());
    session.asked.insert("call_2_1".to_string());
    session.status(&waiting);
    assert_eq!(session.asked.len(), 1, "了结的忘掉，在等的留着");
    assert!(session.status(&json!({})).is_empty());
    assert!(session.asked.is_empty());
}

#[test]
fn a_title_is_reported_once() {
    let mut session = Session::new("/work");
    let status = json!({"title": "修 bug"});
    assert_eq!(
        session.status(&status),
        [Action::Update(
            json!({"sessionUpdate": "session_info_update", "title": "修 bug"})
        )]
    );
    assert!(session.status(&status).is_empty());
}
