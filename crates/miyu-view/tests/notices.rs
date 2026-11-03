//! 旁白：手动压缩、后台任务了结、一组题答了（施工 9-8 上）；翻页时派在更早一页的任务（9-8 中）。

use std::sync::Arc;

use miyu_kernel::event::{Choice, ErrorClass, JobReason, Question, Response};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Policy, Shorten};
use miyu_kernel::testkit::{Line, Play};
use miyu_view::Projector;

use crate::support::*;

#[test]
fn a_manual_compaction_is_one_notice_without_an_end() {
    let mut stage = stage();
    stage.limits(Some(200_000), Some(8_000));
    stage.model([Line::says("one")]);
    stage.say("first");
    summarizes(&mut stage, "summary of the talk");
    stage.request_compaction(Some("keep the names"));
    let entries = same(&stage);
    let notice = json(entries.last().expect("有条目"));
    assert_eq!(notice["what"], "compaction", "{entries:#?}");
    assert_eq!(notice["state"], "done");
    assert_eq!(notice["trigger"], "manual");
    assert_eq!(notice["instructions"], "keep the names");
    assert!(
        notice["took_ms"].is_u64(),
        "那一轮的用时接在这一条上：{notice}"
    );
    assert_eq!(of_kind(&entries, "end").len(), 1, "压缩那一轮不另起收尾");
}

/// 压缩线很低、摘要请求报超长会截短的策略（施工 UP-2）。
fn shortening() -> Policy {
    let mut policy = policy();
    if let Some(compaction) = policy.compaction.as_mut() {
        compaction.reserve_cap = 10;
        compaction.margin = 10;
        compaction.price = miyu_kernel::estimate::Flat {
            image: 50,
            file: 50,
        };
        compaction.shorten = Some(Shorten {
            tries: 3,
            percent: 20,
        });
    }
    policy
}

#[test]
fn a_compaction_that_cut_the_oldest_part_says_how_many_groups() {
    let mut stage = policy_stage(shortening);
    // 第一轮调两次工具再答：四组。第二轮的主请求报超长，被动压缩；摘要请求也报超长，截掉最老的一组再发。
    stage.model([
        Line::calls("", &[("read", "{}")]),
        Line::calls("", &[("read", "{}")]),
        Line::says("好。"),
    ]);
    stage.tools([Play::Done("a".to_string()), Play::Done("b".to_string())]);
    stage.say("看看");
    stage.limits(Some(200_000), None);
    stage.model([
        Line::fails(ErrorClass::ContextTooLong, "413"),
        Line::fails(ErrorClass::ContextTooLong, "413"),
        Line::says("summary"),
        Line::says("嗯。"),
    ]);
    stage.say("再说");
    let entries = same(&stage);
    let compaction: Vec<_> = entries
        .iter()
        .map(json)
        .filter(|entry| entry["what"] == "compaction")
        .collect();
    assert_eq!(compaction.len(), 1, "{entries:#?}");
    assert_eq!(compaction[0]["state"], "done");
    assert_eq!(compaction[0]["truncated"], 1, "{}", compaction[0]);
}

#[test]
fn a_background_command_reports_with_its_title_and_command() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[(
                "shell",
                r#"{"command":"cargo build","description":"Build","run_in_background":true}"#,
            )],
        ),
        Line::says("Started."),
        Line::says("Build finished."),
    ]);
    stage.tools([Play::starts_command(1, "Build")]);
    stage.say("build");
    stage.job_ends(1, JobReason::Exited, By::Kernel, None);
    let entries = same(&stage);
    let tool = json(of_kind(&entries, "tool")[0]);
    assert_eq!(tool["job"], "j1");
    let notice = json(
        entries
            .iter()
            .rev()
            .find(|e| json(e)["what"] == "job")
            .expect("有任务的旁白"),
    );
    assert_eq!(notice["title"], "Build");
    assert_eq!(notice["job_kind"], "command");
    assert_eq!(notice["mark"], "done");
    assert_eq!(notice["command"], "cargo build");
    assert_eq!(notice["output"]["chars"], 48_213);
    // 施工 9-8 三补：退出码、用时照回报，没有信号的不写。
    assert_eq!(notice["exit_code"], 0, "{notice}");
    assert_eq!(notice["took_ms"], 81_234);
    assert!(notice.get("signal").is_none(), "{notice}");
}

/// 监视的命令出的一批行（施工 MN-1 下，设计 56 第四节第 4 条）：一条 `lines` 旁白，带编号、标题、命令原文、这一批全文、
/// 几行、截掉几个字；不是任务了结，状态栏那一行照旧在跑。
#[test]
fn a_watched_command_shows_its_lines() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[(
                "shell",
                r#"{"command":"tail -f log","description":"Watch log","watch":true}"#,
            )],
        ),
        Line::says("Watching."),
        Line::says("Saw it."),
    ]);
    stage.tools([Play::starts_command(1, "Watch log")]);
    stage.say("watch");
    stage.job_lines(1, "ERROR disk full\nERROR again", 2, By::Kernel, None);
    let entries = same(&stage);
    let notice = json(
        entries
            .iter()
            .rev()
            .find(|e| json(e)["what"] == "lines")
            .expect("有一批行的旁白"),
    );
    assert_eq!(notice["job"], "j1");
    assert_eq!(notice["title"], "Watch log");
    assert_eq!(notice["command"], "tail -f log");
    assert_eq!(notice["text"], "ERROR disk full\nERROR again");
    assert_eq!(notice["lines"], 2);
    assert_eq!(notice["omitted"], 0);
    assert!(
        !entries.iter().any(|e| json(e)["what"] == "job"),
        "不是任务了结"
    );
}

/// 翻页时任务派在更早的一页（施工 9-8 中）：先照切点前的日志学任务，这一页的回报照样有标题和命令。
#[test]
fn a_page_cut_after_the_start_still_names_the_job() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[(
                "shell",
                r#"{"command":"cargo build","description":"Build","run_in_background":true}"#,
            )],
        ),
        Line::says("Started."),
        Line::says("Build finished."),
    ]);
    stage.tools([Play::starts_command(1, "Build")]);
    stage.say("build");
    stage.job_ends(1, JobReason::Exited, By::Kernel, None);
    let log = stage.log();
    let cut = log
        .iter()
        .position(|event| matches!(event.body, miyu_kernel::event::Body::JobReported(_)))
        .expect("有回报");
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    projector.learn(&log[..cut]);
    for event in &log[cut..] {
        projector.event(event);
    }
    let entries = projector.entries();
    assert!(
        of_kind(entries, "tool").is_empty(),
        "派它的那一步在更早的一页"
    );
    let notice = json(
        entries
            .iter()
            .find(|e| json(e)["what"] == "job")
            .expect("有任务的旁白"),
    );
    assert_eq!(notice["title"], "Build");
    assert_eq!(notice["job_kind"], "command");
    assert_eq!(notice["command"], "cargo build");
}

#[test]
fn answered_questions_carry_what_was_asked() {
    let mut stage = stage();
    let question = Question {
        header: None,
        question: "Which one?".to_string(),
        options: vec![
            Choice {
                label: "A".to_string(),
                description: None,
                preview: None,
            },
            Choice {
                label: "B".to_string(),
                description: None,
                preview: None,
            },
        ],
        multiple: false,
    };
    stage.model([
        Line::calls("", &[("ask_user", r#"{"questions":[]}"#)]),
        Line::says("Thanks."),
    ]);
    stage.tools([Play::Asks(vec![question])]);
    stage.say("ask me");
    // 问的时候那一步就带上题目（施工 9-8 十四补，终端的抽屉要）。
    let asking = page(&stage);
    let step = json(of_kind(&asking, "tool")[0]);
    assert_eq!(step["questions"][0]["question"], "Which one?", "{step}");
    let call = stage.ran()[0].0;
    stage.reply(
        call,
        vec![Response {
            picked: vec!["A".to_string()],
            text: None,
            notes: None,
            attachments: Vec::new(),
        }],
    );
    let entries = same(&stage);
    let notice = json(
        entries
            .iter()
            .find(|e| json(e)["what"] == "answered")
            .expect("有答了的旁白"),
    );
    assert_eq!(notice["questions"][0]["question"], "Which one?");
    assert_eq!(notice["answers"][0]["picked"][0], "A");
    let step = json(of_kind(&entries, "tool")[0]);
    assert_eq!(
        step["questions"][0]["options"][1]["label"], "B",
        "答了照旧留着"
    );
}
