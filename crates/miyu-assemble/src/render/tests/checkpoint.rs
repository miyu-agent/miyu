//! 检查点那一块：排在最前、摘要不转义、代码写的几段和重读的文件跟在摘要后面、取不到原文的那一份不写、被动压缩留下的
//! 尾巴跟在检查点后面（施工 AS-10 从 `render/tests.rs` 挪出来，那个文件到了 500 行）。

use super::*;

#[test]
fn the_checkpoint_comes_first_and_the_summary_is_not_escaped() {
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    log.reply(&format!("[{}]", text_json("hello")));
    log.end("completed");
    let summary = "Alice said \"hi\".\nNothing <b>& in</b> progress.";
    // 回合开头就压（压缩带着它所在的回合，施工 6-9）：替代到上一轮为止。
    let upto = log.next() - 1;
    let again = log.say("go on");
    log.start(again);
    log.compact(upto, summary);
    log.fact("<env/>");
    let messages = render(log.history(), &texts());
    assert_eq!(
        messages,
        [Message::User {
            blocks: vec![
                text(&format!("<checkpoint>\n{summary}\n</checkpoint>\n")),
                text("<env/>"),
                text("go on"),
            ],
        }]
    );
}

#[test]
fn notes_and_reread_files_follow_the_summary_inside_the_checkpoint() {
    // 施工 6-5：摘要、收尾、代码写的几段、每个重读的文件（原文不转义）、包装的结尾，拼成一块。
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    log.reply(&format!("[{}]", text_json("hello")));
    let file = "fn a() { \"x\" }\n";
    log.compact_rebuilt(
        log.next() - 1,
        "S",
        "<notes/>\n",
        &[("src/a.rs", file), ("b.rs", "b")],
        true,
    );
    log.end("completed");
    let mut texts = texts();
    texts.checkpoint_end = "<end/>\n".to_string();
    let messages = render(log.history(), &texts);
    assert_eq!(
        messages,
        [Message::User {
            blocks: vec![text(&format!(
                "<checkpoint>\nS\n</checkpoint>\n<notes/>\n<file src/a.rs>\n{file}\n</file>\n<file b.rs>\nb\n</file>\n<end/>\n"
            ))],
        }]
    );
}

#[test]
fn a_reread_file_whose_text_is_missing_is_left_out() {
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    log.compact_rebuilt(log.next() - 1, "S", "", &[("a.rs", "a")], false);
    log.end("completed");
    let messages = render(log.history(), &texts());
    assert_eq!(
        messages,
        [Message::User {
            blocks: vec![text("<checkpoint>\nS\n</checkpoint>\n")],
        }]
    );
}

#[test]
fn the_tail_kept_by_a_passive_compaction_follows_the_checkpoint() {
    let mut log = Log::new();
    let hi = log.say("look at src");
    log.start(hi);
    log.fact("<env/>");
    let upto = log.next() - 1;
    let call = log.reply_calling("looking");
    log.result(&call, "ok", "lib.rs");
    log.compact(upto, "Alice asked to look at src.");
    assert_eq!(
        rendered(&log),
        [
            "user: <checkpoint>\nAlice asked to look at src.\n</checkpoint>\n".to_string(),
            "assistant: looking | [call read]".to_string(),
            format!("tool {call} ok: lib.rs"),
        ]
    );
}
