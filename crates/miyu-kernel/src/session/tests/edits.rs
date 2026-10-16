//! 撤销、恢复、重做不再改回文件（施工 U-1，2026-10-15 项目主人定：改回文件交给 git 这类程序）：撤掉的那一轮改过文件的，
//! 照样只记撤销那一条，落了盘就回应，不交出别的动作、不挡命令、照常空闲；以前日志里的 `files.restored` 照样载入。

use super::executor::*;
use super::load::{Logged, load};
use super::revert::{revert, unrevert};
use super::*;
use crate::event::{Effect, FileChanged, FilesRestored, RestoreAction, RestoreOutcome, Restored};
use crate::id::{CallId, ContentHash};

/// 一段内容的哈希。
fn hash(text: &str) -> ContentHash {
    ContentHash::of(text.as_bytes())
}

/// 调用 `call_id` 执行完了，把 `/w/a.txt` 从 A 改成了 B。
pub(super) fn changed(call_id: CallId) -> Input {
    Input::ToolDone {
        at: at(50),
        call_id,
        error: false,
        blocks: Vec::new(),
        duration_ms: Some(3),
        human: None,
        effects: vec![Effect::FileChanged(FileChanged {
            path: "/w/a.txt".to_string(),
            before: Some(hash("A")),
            after: hash("B"),
        })],
        stopped: false,
        end_turn: false,
        sent: Vec::new(),
        display_name: Default::default(),
    }
}

/// 一轮（3 号）：她调了一次 `write`，把 `/w/a.txt` 从 A 改成了 B，说完了，全落了盘。交回工具结果的序号。
pub(super) fn edited() -> (Logged, Seq) {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "改一下");
    logged.tools(seen, &[("write", r#"{"file_path":"a.txt"}"#)]);
    let reply = logged
        .log
        .iter()
        .rev()
        .find(|event| matches!(event.body, Body::MessageAssistant(_)))
        .map(|event| event.seq)
        .unwrap();
    logged.handle(changed(call(reply.get(), 1)));
    let result = logged
        .log
        .iter()
        .rev()
        .find(|event| matches!(event.body, Body::ToolResult(_)))
        .map(|event| event.seq)
        .unwrap();
    let actions = logged.handle(stored(logged.last()));
    let (seen, _) = calls(&actions).remove(0);
    logged.say(seen.get(), "改好了");
    (logged, result)
}

/// 动作里只有追加：没有别的交给执行器去做的。
fn only_appends(actions: &[Action]) -> bool {
    actions
        .iter()
        .all(|action| matches!(action, Action::Append(_)))
}

#[test]
fn an_undo_that_changed_files_only_takes_back_the_talk() {
    let (mut logged, _) = edited();
    let actions = logged.handle(revert(9, 3));
    let reverted = logged.last();
    assert_eq!(appended(&actions), seqs(&[reverted]), "只记撤销");
    assert!(only_appends(&actions), "文件不动：{actions:?}");
    let actions = logged.handle(stored(reverted));
    assert_eq!(
        replies(&actions),
        [&accepted_reply(9, &[reverted])],
        "落了盘就回应"
    );
    assert!(logged.session.idle(), "照常空闲");
    let actions = logged.handle(unrevert(10));
    let unreverted = logged.last();
    assert_eq!(appended(&actions), seqs(&[unreverted]), "恢复也只记那一条");
    assert!(only_appends(&actions), "文件不动：{actions:?}");
    let actions = logged.handle(stored(unreverted));
    assert_eq!(replies(&actions), [&accepted_reply(10, &[unreverted])]);
    assert!(logged.session.idle());
}

#[test]
fn a_command_right_after_the_undo_is_taken() {
    let (mut logged, _) = edited();
    logged.handle(revert(9, 3));
    logged.handle(stored(logged.last()));
    let actions = logged.handle(send(10, "hi"));
    assert!(
        replies(&actions).is_empty() && !appended(&actions).is_empty(),
        "不挡命令：{actions:?}"
    );
}

#[test]
fn an_old_log_with_restored_files_still_loads() {
    let (mut logged, result) = edited();
    logged.handle(revert(9, 3));
    logged.handle(stored(logged.last()));
    let mut log = logged.log.clone();
    let last = log.last().expect("有日志").clone();
    let files = vec![Restored {
        result,
        effect: 0,
        path: "/w/a.txt".to_string(),
        action: RestoreAction::Write,
        outcome: RestoreOutcome::Restored,
        hash: None,
        found: None,
        trash: None,
        error: None,
    }];
    log.push(Event {
        seq: Seq::new(last.seq.get() + 1).expect("从 1 数起"),
        body: Body::FilesRestored(FilesRestored { files }),
        ..last
    });
    let (mut session, _) = load(log);
    assert!(session.idle(), "以前改回文件的那一条照样认得");
    let actions = session.handle(unrevert(10));
    assert!(only_appends(&actions), "恢复照样只记那一条：{actions:?}");
}
