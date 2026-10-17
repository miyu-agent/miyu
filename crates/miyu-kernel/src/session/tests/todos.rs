//! 待办（施工 D-3，`docs/blueprint/kernel/session.md`「待办」）：换了一份、落了盘才推 `todos.changed`，和上次一样的不推；
//! 撤销写它的那一轮退回更早的一份，恢复回来；载入以后照日志算回来，不重推。

use super::executor::*;
use super::load::{Logged, load};
use super::revert::{revert, unrevert};
use super::*;
use crate::event::{Effect, Todo, TodoStatus, TodoWritten, TodosCleared, Transient, TransientBody};
use crate::id::CallId;

fn list(items: &[(&str, TodoStatus)]) -> Vec<Todo> {
    items
        .iter()
        .map(|(content, status)| Todo {
            content: content.to_string(),
            status: status.clone(),
        })
        .collect()
}

/// 调用 `call_id` 跑完了，换上 `written`。
fn wrote(call_id: CallId, written: TodoWritten) -> Input {
    Input::ToolDone {
        at: at(50),
        call_id,
        error: false,
        blocks: vec![Block::Text(Text {
            text: "updated".to_string(),
        })],
        duration_ms: Some(1),
        human: None,
        effects: vec![Effect::TodoWritten(written)],
        stopped: false,
        end_turn: false,
        sent: Vec::new(),
        display_name: Default::default(),
    }
}

/// 推给头的 `todos.changed`，照先后。
fn changes(actions: &[Action]) -> Vec<Vec<Todo>> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::PushTransient(Transient {
                body: TransientBody::TodosChanged(changed),
                ..
            }) => Some(changed.todos.clone()),
            _ => None,
        })
        .collect()
}

/// 推给头的 `todos.changed` 带的做完的清单（施工 D-3 补），照先后。
fn dones(actions: &[Action]) -> Vec<Vec<Todo>> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::PushTransient(Transient {
                body: TransientBody::TodosChanged(changed),
                ..
            }) => Some(changed.done.clone()),
            _ => None,
        })
        .collect()
}

/// 请求 `seen` 调一次 `todowrite`，换上 `todos`，都落了盘：交回这一路推的 `todos.changed` 和下一次请求的 `seen`。
fn write(logged: &mut Logged, seen: u64, todos: Vec<Todo>) -> (Vec<Vec<Todo>>, u64) {
    let (actions, seen) = written(
        logged,
        seen,
        TodoWritten {
            todos,
            done: Vec::new(),
        },
    );
    (changes(&actions), seen)
}

/// 同 [`write`]，换上的是 `written`，交回这一路推的动作。
fn written(logged: &mut Logged, seen: u64, written: TodoWritten) -> (Vec<Action>, u64) {
    // 测试的策略里工具面只有常用的几件：工具名不要紧，内核只看效果。
    let actions = call_tools(&mut logged.session, seen, &[("write", "{}")]);
    logged.log.extend(appended_events(&actions));
    let actions = logged.allowing(stored(logged.last()));
    let call_id = ran(&actions)[0];
    let mut pushed = logged.handle(wrote(call_id, written));
    let actions = logged.handle(stored(logged.last()));
    let seen = calls(&actions).remove(0).0.get();
    pushed.extend(actions);
    (pushed, seen)
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

#[test]
fn a_new_list_is_pushed_once_it_is_stored_and_the_same_one_is_not() {
    let a = list(&[
        ("读代码", TodoStatus::Completed),
        ("写测试", TodoStatus::InProgress),
    ]);
    let mut logged = Logged::new();
    let seen = logged.ask(1, "开工");
    let (pushed, seen) = write(&mut logged, seen, a.clone());
    assert_eq!(pushed, std::slice::from_ref(&a), "落了盘才推，推一次");
    assert_eq!(logged.session.todos(), a);
    let (pushed, seen) = write(&mut logged, seen, a.clone());
    assert!(pushed.is_empty(), "和上次一样的不推");
    let (pushed, _) = write(&mut logged, seen, Vec::new());
    assert_eq!(pushed, [Vec::<Todo>::new()], "清空了也推");
    assert!(logged.session.todos().is_empty());
}

#[test]
fn undoing_goes_back_to_the_earlier_list_and_redoing_restores_it() {
    let a = list(&[("一", TodoStatus::Pending)]);
    let b = list(&[("一", TodoStatus::Completed), ("二", TodoStatus::Pending)]);
    let mut logged = Logged::new();
    let seen = logged.ask(1, "第一轮");
    let (_, seen) = write(&mut logged, seen, a.clone());
    logged.say(seen, "好");
    let seen = logged.ask(2, "第二轮");
    let second = last_turn(&logged);
    let (_, seen) = write(&mut logged, seen, b.clone());
    logged.say(seen, "好");
    assert_eq!(logged.session.todos(), b);
    logged.handle(revert(3, second));
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(
        changes(&actions),
        std::slice::from_ref(&a),
        "撤掉第二轮，退回第一轮写的"
    );
    assert_eq!(logged.session.todos(), a);
    logged.handle(unrevert(4));
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(changes(&actions), std::slice::from_ref(&b), "恢复回来");
}

#[test]
fn a_loaded_session_counts_the_list_back_without_pushing_it() {
    let a = list(&[("一", TodoStatus::InProgress)]);
    let mut logged = Logged::new();
    let seen = logged.ask(1, "开工");
    let (_, seen) = write(&mut logged, seen, a.clone());
    logged.say(seen, "好");
    let (session, actions) = load(logged.log.clone());
    assert_eq!(session.todos(), a);
    assert!(changes(&actions).is_empty(), "头订阅时从回应里拿，载入不推");
    // 载入以后接着说一轮：清单没变，照样不推。
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

/// 一步里调了两件：第一件的结果先落了盘，换待办的第二件还没落盘，这时不推；它也落了盘才推（先落盘，后推送）。
#[test]
fn a_list_not_yet_on_disk_is_not_pushed() {
    let a = list(&[("一", TodoStatus::Pending)]);
    let mut logged = Logged::new();
    let seen = logged.ask(1, "开工");
    // 两件只读的一起派出去（工具名不要紧，内核只看效果）。
    let actions = call_tools(&mut logged.session, seen, &[("read", "{}"), ("read", "{}")]);
    logged.log.extend(appended_events(&actions));
    let actions = logged.allowing(stored(logged.last()));
    let ran = ran(&actions);
    logged.handle(done(ran[0], "read"));
    let first = logged.last();
    logged.handle(wrote(
        ran[1],
        TodoWritten {
            todos: a.clone(),
            done: Vec::new(),
        },
    ));
    let actions = logged.handle(stored(first));
    assert!(changes(&actions).is_empty(), "换待办的那一条还没落盘");
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(changes(&actions), std::slice::from_ref(&a));
}

/// 因为全部做完而清空的那一条带上做完的清单（施工 D-3 补）：头看得到最后一项打勾。撤销那一轮退回更早的一份，不带；恢复回来
/// 又带。她写一份空的（不是因为做完）不带。
#[test]
fn clearing_because_all_is_done_carries_the_finished_list() {
    let a = list(&[("一", TodoStatus::InProgress)]);
    let finished = list(&[("一", TodoStatus::Completed)]);
    let mut logged = Logged::new();
    let seen = logged.ask(1, "第一轮");
    let (_, seen) = write(&mut logged, seen, a.clone());
    logged.say(seen, "好");
    let seen = logged.ask(2, "第二轮");
    let second = last_turn(&logged);
    let (actions, seen) = written(
        &mut logged,
        seen,
        TodoWritten {
            todos: Vec::new(),
            done: finished.clone(),
        },
    );
    assert_eq!(changes(&actions), [Vec::<Todo>::new()]);
    assert_eq!(dones(&actions), std::slice::from_ref(&finished));
    logged.say(seen, "好");
    logged.handle(revert(3, second));
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(changes(&actions), std::slice::from_ref(&a));
    assert_eq!(dones(&actions), [Vec::<Todo>::new()], "退回去的不带");
    logged.handle(unrevert(4));
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(
        dones(&actions),
        std::slice::from_ref(&finished),
        "恢复回来又带"
    );
    let seen = logged.ask(5, "第三轮");
    let (_, seen) = write(&mut logged, seen, a.clone());
    let (actions, _) = written(
        &mut logged,
        seen,
        TodoWritten {
            todos: Vec::new(),
            done: Vec::new(),
        },
    );
    assert_eq!(dones(&actions), [Vec::<Todo>::new()], "写空的不带");
}

/// 编号是 `n` 的命令：alice 要清空待办（施工 D-3 再补，`/todo clear`）。
fn clear_todos(n: u64) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(n % 60),
        command: Command::ClearTodos,
    })
}

/// 清空上下文（`/clear`）连待办一起清（施工 D-3 再补，2026-10-17 项目主人定）：落了盘推一份空的；撤掉清空的那一轮，清单回来，
/// 恢复又清掉。
#[test]
fn clearing_the_context_clears_the_list_and_undoing_brings_it_back() {
    let a = list(&[("一", TodoStatus::InProgress)]);
    let mut logged = Logged::new();
    let seen = logged.ask(1, "开工");
    let (_, seen) = write(&mut logged, seen, a.clone());
    logged.say(seen, "好");
    logged.handle(super::clear::clear(2));
    let cleared = last_turn(&logged);
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(
        changes(&actions),
        [Vec::<Todo>::new()],
        "清空上下文也清待办"
    );
    assert!(logged.session.todos().is_empty());
    let (session, _) = load(logged.log.clone());
    assert!(session.todos().is_empty(), "载入以后照样是空的");
    // 撤掉压缩要先读回日志（施工 6-9）：照它重建有效历史。
    let actions = logged.handle(revert(3, cleared));
    let [Action::ReadBack { from }] = actions.as_slice() else {
        panic!("先读回：{actions:?}");
    };
    let from = *from;
    let events = logged
        .log
        .iter()
        .filter(|event| event.seq >= from)
        .cloned()
        .collect();
    logged.handle(Input::ReadBack {
        at: at(58),
        from,
        events,
    });
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(
        changes(&actions),
        std::slice::from_ref(&a),
        "撤掉清空，清单回来"
    );
    logged.handle(unrevert(4));
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(changes(&actions), [Vec::<Todo>::new()], "恢复又清掉");
}

/// `/todo clear`（施工 D-3 再补）：记一条 `todos.cleared`（谁发的写在 `by`，`cause` 是命令），落了盘推一份空的、回应附上
/// 这一条；载入以后照样是空的。
#[test]
fn the_todo_command_records_a_clear_and_pushes_an_empty_list() {
    let a = list(&[("一", TodoStatus::Pending)]);
    let mut logged = Logged::new();
    let seen = logged.ask(1, "开工");
    let (_, seen) = write(&mut logged, seen, a);
    logged.say(seen, "好");
    let actions = logged.handle(clear_todos(2));
    let [Action::Append(events)] = actions.as_slice() else {
        panic!("只追加一条：{actions:?}");
    };
    let [event] = events.as_slice() else {
        panic!("一条：{events:?}");
    };
    assert_eq!(event.body, Body::TodosCleared(TodosCleared {}));
    assert_eq!(event.by, alice());
    assert_eq!(event.cause, Some(id(2)));
    assert_eq!(event.turn, None, "闲着时不属于哪一轮");
    let seq = event.seq;
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(changes(&actions), [Vec::<Todo>::new()]);
    assert!(actions.contains(&Action::Reply {
        id: id(2),
        outcome: Outcome::Accepted { events: vec![seq] },
    }));
    let (session, _) = load(logged.log.clone());
    assert!(session.todos().is_empty(), "载入以后照样是空的");
}

/// 回合进行中照收：这一条带上这一轮，撤掉那一轮清单回来。
#[test]
fn the_todo_command_works_while_a_turn_runs() {
    let a = list(&[("一", TodoStatus::Pending)]);
    let mut logged = Logged::new();
    let seen = logged.ask(1, "开工");
    let turn = last_turn(&logged);
    let (_, _) = write(&mut logged, seen, a);
    let actions = logged.handle(clear_todos(2));
    let [Action::Append(events)] = actions.as_slice() else {
        panic!("只追加一条：{actions:?}");
    };
    assert_eq!(events[0].turn, Some(TurnId::new(seq(turn))));
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(changes(&actions), [Vec::<Todo>::new()]);
}

/// 没有待办的拒绝，`nothing_to_clear`，什么都不追加：没写过的、清空了的都是。
#[test]
fn clearing_no_list_is_refused() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "开工");
    logged.say(seen, "好");
    let refused = Action::Reply {
        id: id(2),
        outcome: Outcome::Rejected {
            reason: Reason::NothingToClear,
            about: Vec::new(),
        },
    };
    assert_eq!(logged.handle(clear_todos(2)), [refused], "没写过");
    let seen = logged.ask(3, "再来");
    let (_, seen) = write(&mut logged, seen, Vec::new());
    logged.say(seen, "好");
    let refused = Action::Reply {
        id: id(4),
        outcome: Outcome::Rejected {
            reason: Reason::NothingToClear,
            about: Vec::new(),
        },
    };
    assert_eq!(logged.handle(clear_todos(4)), [refused], "清空了的");
}

/// 清空那一批三条只落了第一条时不推，清空的检查点也落了盘才推（先落盘，后推送；施工 D-3 再补）。
#[test]
fn a_clear_not_yet_on_disk_is_not_pushed() {
    let a = list(&[("一", TodoStatus::Pending)]);
    let mut logged = Logged::new();
    let seen = logged.ask(1, "开工");
    let (_, seen) = write(&mut logged, seen, a);
    logged.say(seen, "好");
    let before = logged.last();
    logged.handle(super::clear::clear(2));
    let actions = logged.handle(stored(before + 1));
    assert!(changes(&actions).is_empty(), "检查点还没落盘：{actions:?}");
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(changes(&actions), [Vec::<Todo>::new()]);
}
