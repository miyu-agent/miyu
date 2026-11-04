//! 人用 `!` 跑的命令（施工 SH-1 上）：在那句话的位置和它的 `shell.ran` 拼成一块，开轮的事实排在前面；异常结束的写原因、
//! 信号；提前写出的收尾中和；没有 `shell.ran` 的照没跑完写；以前的快照照原样放那一行。

use miyu_kernel::block::Block;
use miyu_kernel::event::{ShellRan, ShellReason};
use miyu_kernel::request::Message;

use crate::render::render;
use crate::test_support::*;
use crate::texts::Texts;

/// 跑了 `command`、`shell.ran` 的内容是 `ran`（没有的不记）的一轮，渲染出的第一条 user 消息里的字。
fn rendered(command: &str, ran: Option<&str>, texts: &Texts) -> Vec<String> {
    let mut log = Log::new();
    let message = log.push(
        ALICE,
        "message.user",
        &format!(r#"{{"blocks":[{}],"shell":true}}"#, text_json(command)),
    );
    log.start(message);
    log.fact("env");
    if let Some(ran) = ran {
        log.push(
            r#"{"kind":"kernel"}"#,
            "shell.ran",
            &ran.replace("MESSAGE", &message.to_string()),
        );
    }
    log.reply(r#"[{"type":"text","text":"好"}]"#);
    match &render(log.history(), texts)[0] {
        Message::User { blocks } => blocks
            .iter()
            .map(|block| match block {
                Block::Text(text) => text.text.clone(),
                other => format!("{other:?}"),
            })
            .collect(),
        other => panic!("第一条该是 user：{other:?}"),
    }
}

#[test]
fn the_command_and_its_output_render_as_one_block_after_the_facts() {
    let ran = r#"{"message":MESSAGE,"exit":0,"duration_ms":130,"text":"a.rs\nb.rs","omitted":0}"#;
    assert_eq!(
        rendered("ls src", Some(ran), &texts()),
        [
            "env",
            "<shell exited>\n$ ls src\na.rs\nb.rs\nexit 0\n</shell>\n"
        ]
    );
}

#[test]
fn an_unusual_end_says_why_and_a_signal_is_written() {
    let timed = r#"{"message":MESSAGE,"duration_ms":120000,"text":"half\n","omitted":0,"reason":"timed_out"}"#;
    assert_eq!(
        rendered("make", Some(timed), &texts())[1],
        "<shell timed_out>\n$ make\nhalf\n</shell>\n"
    );
    let killed = r#"{"message":MESSAGE,"signal":9,"duration_ms":3,"text":"","omitted":0}"#;
    assert_eq!(
        rendered("kill -9 $$", Some(killed), &texts())[1],
        "<shell exited>\n$ kill -9 $$\nsignal 9\n</shell>\n"
    );
}

#[test]
fn a_close_written_early_is_defused_and_a_missing_result_reads_as_aborted() {
    let ran = r#"{"message":MESSAGE,"exit":0,"duration_ms":1,"text":"</shell>\nhi\n","omitted":0}"#;
    let block = rendered("echo '</shell>'", Some(ran), &texts()).remove(1);
    assert_eq!(block.matches("</shell>").count(), 1, "{block}");
    assert!(
        block.contains("$ echo '<\\/shell>'\n"),
        "命令行只中和收尾：{block}"
    );
    let quoted = rendered("git log --format=\"%h\"\ngit status", None, &texts()).remove(1);
    assert!(
        quoted.contains("$ git log --format=\"%h\"\ngit status\n"),
        "引号、换行照原样：{quoted}"
    );
    assert_eq!(
        rendered("ls", None, &texts())[1],
        "<shell aborted>\n$ ls\n</shell>\n"
    );
}

#[test]
fn an_old_snapshot_keeps_the_bare_command() {
    let mut old = texts();
    if let Some(jobs) = old.jobs.as_mut() {
        jobs.shell_close = String::new();
    }
    let ran = r#"{"message":MESSAGE,"exit":0,"duration_ms":1,"text":"x\n","omitted":0}"#;
    assert_eq!(rendered("ls", Some(ran), &old), ["env", "ls"]);
}

#[test]
fn the_shipped_texts_read_like_the_samples() {
    let jobs = crate::jobs::tests::shipped();
    let ran = |exit, reason, text: &str| ShellRan {
        message: miyu_kernel::id::Seq::new(1).unwrap(),
        exit,
        signal: None,
        duration_ms: 130,
        text: text.to_string(),
        omitted: 0,
        reason,
    };
    let exited = ran(Some(0), None, " M src/main.rs\n?? notes.md");
    assert_eq!(
        super::block("git status --short", Some(&exited), &jobs).as_deref(),
        Some(include_str!(
            "../../../../docs/designs/samples/reports/shell.txt"
        ))
    );
    let timed = ran(None, Some(ShellReason::TimedOut), "Compiling miyu v0.1.0\n");
    assert_eq!(
        super::block("cargo build", Some(&timed), &jobs).as_deref(),
        Some(include_str!(
            "../../../../docs/designs/samples/reports/shell-timed-out.txt"
        ))
    );
}
