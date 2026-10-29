//! `miyu schedule` 印的样子、读的时刻和间隔、`add` 交的参数（`docs/blueprint/cli/schedule.md`）。时区照测试交的，不看这台机器。

use std::path::PathBuf;

use clap::Parser;
use jiff::tz::TimeZone;
use serde_json::json;

use super::shown::{interval, line, moment};
use super::{Schedule, ScheduleCommand, SchedulePlan, added};
use crate::language::Language;

fn tokyo() -> TimeZone {
    TimeZone::get("Asia/Tokyo").expect("认得东京")
}

#[test]
fn one_line_each_in_the_local_zone() {
    let cron = json!({
        "id": "t1", "when": {"cron": "0 8 * * *", "recurring": true, "zone": "Asia/Tokyo"},
        "prompt": "早报：天气和日程", "target": {"new": {"persona": "miyu", "cwd": "/home/me"}},
        "next": "2026-10-27T23:00:00.000Z",
        "last": {"at": "2026-10-26T23:00:00.000Z", "result": "ran"},
    });
    assert_eq!(
        line(&cron, &Language::Chinese, &tokyo()),
        "t1  cron 0 8 * * *（Asia/Tokyo）  下一次 2026-10-28 08:00  上一次 2026-10-27 08:00 已运行  新会话 人格 miyu 目录 /home/me  「早报：天气和日程」"
    );
    let every = json!({
        "id": "t2", "when": {"every": 600}, "prompt": "看一眼服务器", "target": {"session": "s-1"},
        "paused": true, "loop": "fixed",
    });
    assert_eq!(
        line(&every, &Language::English, &TimeZone::UTC),
        "t2  every 10m  loop  paused  session s-1  \"看一眼服务器\""
    );
    let at = json!({
        "id": "t3", "when": {"at": "2026-10-28T00:00:00.000Z"}, "prompt": "提醒我开会",
        "target": {"new": {"persona": null}}, "next": "2026-10-28T00:00:00.000Z",
        "last": {"at": "2026-10-27T00:00:00.000Z", "result": "skipped:session_gone"},
    });
    assert_eq!(
        line(&at, &Language::Chinese, &TimeZone::UTC),
        "t3  at 2026-10-28 00:00  下一次 2026-10-28 00:00  上一次 2026-10-27 00:00 跳过：会话已不存在  新会话 无人格  「提醒我开会」"
    );
    let pushed = json!({
        "id": "t5", "when": {"every": 600}, "prompt": "{}", "target": {"package": {}},
        "by": {"kind": "package", "package": "im"},
        "last": {"at": "2026-10-27T00:00:00.000Z", "result": "skipped:package_unavailable"},
    });
    assert_eq!(
        line(&pushed, &Language::Chinese, &TimeZone::UTC),
        "t5  every 10m  上一次 2026-10-27 00:00 跳过：软件包未运行  发给软件包 im  「{}」"
    );
    let once = json!({"id": "t4", "when": {"cron": "30 7 * * 1", "recurring": false}, "prompt": "x", "target": {"new": {}}});
    assert_eq!(
        line(&once, &Language::English, &TimeZone::UTC),
        "t4  cron 30 7 * * 1 (once)  new  \"x\""
    );
}

#[test]
fn a_long_prompt_is_cut() {
    let long = "一".repeat(50);
    let item = json!({"id": "t1", "when": {"every": 90}, "prompt": long, "target": {"new": {}}});
    let shown = line(&item, &Language::Chinese, &TimeZone::UTC);
    assert!(
        shown.ends_with(&format!("「{}…」", "一".repeat(40))),
        "{shown}"
    );
    assert!(shown.contains("every 90s"), "{shown}");
}

#[test]
fn intervals() {
    assert_eq!(interval("600"), Some(600));
    assert_eq!(interval("90s"), Some(90));
    assert_eq!(interval("10m"), Some(600));
    assert_eq!(interval("2H"), Some(7_200));
    assert_eq!(interval("1d"), Some(86_400));
    for bad in [
        "",
        "m",
        "10x",
        "1.5h",
        "-5",
        "10 m",
        "99999999999999999999d",
    ] {
        assert_eq!(interval(bad), None, "{bad:?}");
    }
}

#[test]
fn moments_become_what_the_core_reads() {
    assert_eq!(
        moment("2026-10-28 08:00", &tokyo()).as_deref(),
        Some("2026-10-27T23:00:00.000Z"),
        "不带偏移的照本机时区"
    );
    assert_eq!(
        moment("2026-10-28T08:00", &tokyo()).as_deref(),
        Some("2026-10-27T23:00:00.000Z")
    );
    assert_eq!(
        moment("2026-10-28T08:00:00+02:00", &tokyo()).as_deref(),
        Some("2026-10-28T06:00:00.000Z"),
        "带偏移的照它"
    );
    assert_eq!(
        moment("2026-10-28T08:00:00.5Z", &tokyo()).as_deref(),
        Some("2026-10-28T08:00:00.500Z")
    );
    for bad in ["tomorrow", "8:00", "2026-13-01 08:00"] {
        assert_eq!(moment(bad, &tokyo()), None, "{bad}");
    }
}

#[derive(Parser)]
struct Line {
    #[command(subcommand)]
    command: Top,
}

#[derive(clap::Subcommand)]
enum Top {
    Schedule(Schedule),
}

fn plan(words: &[&str], zone: TimeZone) -> SchedulePlan {
    let line = ["miyu", "schedule"]
        .into_iter()
        .chain(words.iter().copied());
    let Top::Schedule(args) = Line::try_parse_from(line).expect("读得懂").command;
    SchedulePlan {
        args,
        language: Language::Chinese,
        cwd: PathBuf::from("/nowhere/here"),
        zone,
    }
}

fn add_params(
    words: &[&str],
    zone: TimeZone,
) -> Result<serde_json::Value, crate::diagnostic::Problem> {
    let plan = plan(words, zone);
    let Some(ScheduleCommand::Add(add)) = &plan.args.command else {
        panic!("是 add");
    };
    added(add, &plan)
}

#[test]
fn a_cron_takes_this_machines_zone_unless_one_is_written() {
    assert_eq!(
        add_params(&["add", "--cron", "0 8 * * *", "早报"], tokyo()).expect("成"),
        json!({"when": {"cron": "0 8 * * *", "recurring": true, "zone": "Asia/Tokyo"}, "prompt": "早报", "target": {"new": {}}})
    );
    assert_eq!(
        add_params(
            &[
                "add",
                "--cron",
                "0 8 * * *",
                "--zone",
                "America/New_York",
                "--once",
                "早",
                "报"
            ],
            tokyo()
        )
        .expect("成")["when"],
        json!({"cron": "0 8 * * *", "recurring": false, "zone": "America/New_York"})
    );
    // 本机说不出 IANA 名字的不交，照核心的默认。
    let fixed = TimeZone::fixed(jiff::tz::offset(9));
    assert_eq!(
        add_params(&["add", "--cron", "0 8 * * *", "早报"], fixed).expect("成")["when"],
        json!({"cron": "0 8 * * *", "recurring": true})
    );
}

#[test]
fn at_every_and_the_target() {
    assert_eq!(
        add_params(
            &[
                "add",
                "--at",
                "2026-10-28 08:00",
                "--session",
                "s-1",
                "开会"
            ],
            tokyo()
        )
        .expect("成"),
        json!({"when": {"at": "2026-10-27T23:00:00.000Z"}, "prompt": "开会", "target": {"session": "s-1"}})
    );
    assert_eq!(
        add_params(
            &[
                "add",
                "--every",
                "10m",
                "--persona",
                "miyu",
                "--preset",
                "dev",
                "--cwd",
                "sub",
                "看"
            ],
            tokyo()
        )
        .expect("成"),
        json!({"when": {"every": 600}, "prompt": "看", "target": {"new": {"persona": "miyu", "preset": "dev", "cwd": PathBuf::from("/nowhere/here").join("sub").to_string_lossy()}}})
    );
    assert_eq!(
        add_params(&["add", "--every", "1h", "--no-persona", "看"], tokyo()).expect("成")["target"],
        json!({"new": {"persona": null}})
    );
    assert!(add_params(&["add", "--every", "soon", "看"], tokyo()).is_err());
    assert!(add_params(&["add", "--at", "tomorrow", "看"], tokyo()).is_err());
}

#[test]
fn the_arguments_that_do_not_go_together() {
    let parse = |words: &[&str]| {
        let line = ["miyu", "schedule"]
            .into_iter()
            .chain(words.iter().copied());
        Line::try_parse_from(line).is_ok()
    };
    assert!(!parse(&["add", "看"]), "时间三选一，得写一个");
    assert!(!parse(&[
        "add",
        "--every",
        "10m",
        "--at",
        "2026-10-28 08:00",
        "看"
    ]));
    assert!(
        !parse(&["add", "--every", "10m", "--once", "看"]),
        "--once 只跟 --cron"
    );
    assert!(!parse(&[
        "add",
        "--at",
        "2026-10-28 08:00",
        "--zone",
        "Asia/Tokyo",
        "看"
    ]));
    assert!(!parse(&[
        "add",
        "--every",
        "10m",
        "--session",
        "s-1",
        "--cwd",
        "x",
        "看"
    ]));
    assert!(!parse(&[
        "add",
        "--every",
        "10m",
        "--persona",
        "a",
        "--no-persona",
        "看"
    ]));
    assert!(!parse(&["add", "--every", "10m"]), "话得写");
    assert!(parse(&[]), "不写命令是 list");
    assert!(parse(&["list", "--session", "s-1", "--format", "json"]));
}
