//! 包的会话状态（施工 TW-1，设计 46）：工具结果带的、程序直接写的，落了盘才推 `package.state_changed`，只比给头看的那几格；
//! 撤销写它的那一轮退回更早的一份，恢复回来；空的是清掉；载入以后照日志算回来，不重推。

use super::executor::*;
use super::load::{Logged, load};
use super::revert::{revert, unrevert};
use super::*;
use crate::event::{Effect, PackageState, State, Transient, TransientBody};
use crate::id::CallId;
use crate::raw::RawJson;

/// 包 `package` 的一份：界面块的内容是一段字 `view`，`note` 是 `note`；`view` 空的不写。
fn written(package: &str, view: &str, note: &str) -> PackageState {
    PackageState {
        package: package.to_string(),
        state: State {
            widgets: (!view.is_empty()).then(|| {
                RawJson::of(
                    &serde_json::json!({"main": {"content": [{"type": "text", "text": view}]}}),
                )
                .expect("写得出")
            }),
            note: (!note.is_empty()).then(|| note.to_string()),
            ..State::default()
        },
    }
}

/// 调用 `call_id` 跑完了，带上 `state`。
fn finished(call_id: CallId, state: PackageState) -> Input {
    Input::ToolDone {
        at: at(50),
        call_id,
        error: false,
        blocks: vec![Block::Text(Text {
            text: "updated".to_string(),
        })],
        duration_ms: Some(1),
        human: None,
        effects: vec![Effect::PackageState(state)],
        stopped: false,
        end_turn: false,
        sent: Vec::new(),
        display_name: Default::default(),
        detail: None,
    }
}

/// 推给头的 `package.state_changed`：包、界面块那一段的字（清掉了的是空的），照先后。
fn changes(actions: &[Action]) -> Vec<(String, String)> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::PushTransient(Transient {
                body: TransientBody::PackageStateChanged(changed),
                ..
            }) => {
                let text = changed
                    .state
                    .widgets
                    .as_ref()
                    .and_then(|widgets| {
                        serde_json::from_str::<serde_json::Value>(widgets.get()).ok()
                    })
                    .and_then(|widgets| {
                        widgets["main"]["content"][0]["text"]
                            .as_str()
                            .map(str::to_string)
                    })
                    .unwrap_or_default();
                assert!(changed.state.note.is_none() && changed.state.data.is_none());
                Some((changed.package.clone(), text))
            }
            _ => None,
        })
        .collect()
}

/// 请求 `seen` 调一件工具，结果带上 `state`，都落了盘：交回这一路推的和下一次请求的 `seen`。
fn by_tool(logged: &mut Logged, seen: u64, state: PackageState) -> (Vec<(String, String)>, u64) {
    let actions = call_tools(&mut logged.session, seen, &[("write", "{}")]);
    logged.log.extend(appended_events(&actions));
    let actions = logged.allowing(stored(logged.last()));
    let call_id = ran(&actions)[0];
    let mut pushed = logged.handle(finished(call_id, state));
    let actions = logged.handle(stored(logged.last()));
    let seen = calls(&actions).remove(0).0.get();
    pushed.extend(actions);
    (changes(&pushed), seen)
}

/// 编号是 `n` 的命令：包的程序直接换上 `state`。
fn set_state(n: u64, state: PackageState) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(n % 60),
        command: Command::SetState(state),
    })
}

/// 包 `package` 当前的 `note`。
fn note(logged: &Logged, package: &str) -> Option<String> {
    logged
        .session
        .package_states()
        .get(package)
        .and_then(|state| state.note.clone())
}

/// 最近一条 `turn.started` 的序号：这一轮的编号。
fn last_turn(logged: &Logged) -> u64 {
    logged
        .log
        .iter()
        .rev()
        .find(|event| matches!(event.body, Body::TurnStarted(_)))
        .map(|event| event.seq.get())
        .expect("开过回合")
}

fn pair(package: &str, text: &str) -> (String, String) {
    (package.to_string(), text.to_string())
}

#[test]
fn a_state_is_pushed_once_it_is_stored_and_only_what_heads_see_counts() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "开工");
    let (pushed, seen) = by_tool(&mut logged, seen, written("todo", "2/5", "a"));
    assert_eq!(pushed, [pair("todo", "2/5")], "落了盘才推，推一次");
    assert_eq!(note(&logged, "todo").as_deref(), Some("a"));
    let (pushed, seen) = by_tool(&mut logged, seen, written("todo", "2/5", "b"));
    assert!(pushed.is_empty(), "给头看的没变不推：{pushed:?}");
    assert_eq!(note(&logged, "todo").as_deref(), Some("b"), "记照记");
    let (pushed, _) = by_tool(&mut logged, seen, written("todo", "", ""));
    assert_eq!(pushed, [pair("todo", "")], "清掉了也推");
    assert!(logged.session.package_states().is_empty());
}

#[test]
fn a_program_writes_its_state_between_turns() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "开工");
    logged.say(seen, "好");
    logged.handle(set_state(2, written("plan", "step 1", "")));
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(changes(&actions), [pair("plan", "step 1")]);
    assert!(
        replies(&actions).iter().any(|action| matches!(
            action,
            Action::Reply {
                outcome: Outcome::Accepted { .. },
                ..
            }
        )),
        "落了盘回应"
    );
    assert!(logged.session.idle(), "不开回合");
}

#[test]
fn undoing_goes_back_to_the_earlier_state_and_redoing_restores_it() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "第一轮");
    let (_, seen) = by_tool(&mut logged, seen, written("todo", "a", ""));
    logged.say(seen, "好");
    let seen = logged.ask(2, "第二轮");
    let second = last_turn(&logged);
    let (_, seen) = by_tool(&mut logged, seen, written("todo", "b", ""));
    logged.say(seen, "好");
    logged.handle(revert(3, second));
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(
        changes(&actions),
        [pair("todo", "a")],
        "撤掉第二轮，退回第一轮写的"
    );
    logged.handle(unrevert(4));
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(changes(&actions), [pair("todo", "b")], "恢复回来");
}

#[test]
fn a_loaded_session_counts_the_states_back_without_pushing_them() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "开工");
    let (_, seen) = by_tool(&mut logged, seen, written("todo", "a", "n"));
    logged.say(seen, "好");
    let (session, actions) = load(logged.log.clone());
    assert_eq!(
        session
            .package_states()
            .get("todo")
            .and_then(|state| state.note.as_deref()),
        Some("n")
    );
    assert!(changes(&actions).is_empty(), "载入不推");
    let mut loaded = Logged {
        session,
        log: logged.log.clone(),
    };
    let mut pushed = Vec::new();
    let actions = loaded.handle(send(2, "接着来"));
    pushed.extend(changes(&actions));
    let actions = loaded.handle(stored(loaded.last()));
    pushed.extend(changes(&actions));
    assert!(pushed.is_empty(), "载入时当已经告诉过：{pushed:?}");
}

/// 一步里调了两件：第一件的结果先落了盘，带状态的第二件还没落盘，这时不推；它也落了盘才推（先落盘，后推送）。
#[test]
fn a_state_not_yet_on_disk_is_not_pushed() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "开工");
    let actions = call_tools(&mut logged.session, seen, &[("read", "{}"), ("read", "{}")]);
    logged.log.extend(appended_events(&actions));
    let actions = logged.allowing(stored(logged.last()));
    let ran = ran(&actions);
    logged.handle(done(ran[0], "read"));
    let first = logged.last();
    logged.handle(finished(ran[1], written("todo", "a", "")));
    let actions = logged.handle(stored(first));
    assert!(changes(&actions).is_empty(), "带状态的那一条还没落盘");
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(changes(&actions), [pair("todo", "a")]);
}
