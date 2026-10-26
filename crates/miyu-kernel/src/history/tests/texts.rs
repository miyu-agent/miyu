//! 改了的文件的小内容（施工 V-18，`kernel/history.md`「有效历史收事件」第 9 条）：有效历史收下时去掉
//! `before_text`、`after_text`，只留哈希；留着一切的那一份照原样留着。

use super::*;
use crate::event::Effect;

/// 一个字母 `a` 的内容哈希。
const A: &str = "sha256:ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb";
/// 空内容的哈希。
const EMPTY: &str = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// 人问一句（2），一轮开始（3），回复带一个调用（4），它的结果改了一个文件、带着两边的原文（5）。
fn edited() -> Vec<Event> {
    let effects = format!(
        r#"[{{"kind":"file.changed","path":"/a","before":"{A}","after":"{EMPTY}","before_text":"a","after_text":""}},{{"kind":"file.read","path":"/b","hash":"{A}"}}]"#
    );
    let body = format!(r#"{{"call_id":"call_4_1","status":"ok","blocks":[],"effects":{effects}}}"#);
    vec![
        created(),
        message(2, ALICE),
        event(3, Some(3), KERNEL, "turn.started", r#"{"trigger":2}"#),
        event(4, Some(3), MODEL, "message.assistant", &calls(4, 3, 1)),
        event(5, Some(3), KERNEL, "tool.result", &body),
    ]
}

/// 第 `seq` 条工具结果里改了的那个文件。
fn changed(history: &History, seq: u64) -> crate::event::FileChanged {
    let event = history
        .events()
        .iter()
        .find(|event| event.seq.get() == seq)
        .expect("这一条在");
    let Body::ToolResult(result) = &event.body else {
        panic!("不是工具结果：{event:?}");
    };
    result
        .effects
        .iter()
        .find_map(|effect| match effect {
            Effect::FileChanged(changed) => Some(changed.clone()),
            _ => None,
        })
        .expect("有改了的文件")
}

#[test]
fn the_effective_history_keeps_hashes_but_not_the_texts() {
    let history = feed(edited());
    let changed = changed(&history, 5);
    assert_eq!(changed.before_text, None);
    assert_eq!(changed.after_text, None);
    assert_eq!(changed.path, "/a");
    assert_eq!(changed.before, Some(ContentHash::of(b"a")));
    assert_eq!(changed.after, ContentHash::of(b""));
    // 别的效果、别的事件照原样。
    let Body::ToolResult(result) = &history.events().last().unwrap().body else {
        panic!("最后一条是工具结果");
    };
    assert_eq!(result.effects.len(), 2);
    assert_eq!(seqs(&history), [1, 2, 3, 4, 5]);
}

#[test]
fn the_whole_history_keeps_the_texts() {
    let mut ledger = Ledger::default();
    let mut whole = History::whole();
    for event in edited() {
        ledger.append(&event).unwrap();
        whole.append(event);
    }
    let changed = changed(&whole, 5);
    assert_eq!(changed.before_text.as_deref(), Some("a"));
    assert_eq!(changed.after_text.as_deref(), Some(""));
}
