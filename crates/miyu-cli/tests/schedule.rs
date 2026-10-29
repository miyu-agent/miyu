//! `miyu schedule`（施工 J-3 下，`docs/blueprint/cli/schedule.md`）：在进程里起一个核心，每个子命令走一遍：印什么、退出码；`cron`
//! 交本机的时区、`--zone` 手写的照它；`--cwd` 照当前目录接成真实的位置；核心拒的照原话说，参数错的接上是哪一样；时刻、间隔读不懂
//! 的不连核心就说、退出码 2。

use std::path::Path;
use std::sync::Arc;

use clap::Parser;
use jiff::tz::TimeZone;
use serde_json::Value;

use crate::support::{Asked, Home, Tape, within};
use miyu_cli::language::Language;
use miyu_cli::{Schedule, SchedulePlan, schedule_on};
use miyu_session::testkit::Script;

/// 照主程序的样子读参数：`miyu schedule …`。
#[derive(Parser)]
struct Line {
    #[command(subcommand)]
    command: Top,
}

#[derive(clap::Subcommand)]
enum Top {
    Schedule(Schedule),
}

fn parsed(words: &[&str]) -> Schedule {
    let line = ["miyu", "schedule"]
        .into_iter()
        .chain(words.iter().copied());
    match Line::try_parse_from(line).expect("参数读得懂").command {
        Top::Schedule(schedule) => schedule,
    }
}

/// 在真的套接字上连上核心，照 `words`（`schedule` 后面的）跑一次；界面是中文，本机在东京，当前目录是 `cwd`。
async fn run(home: &Home, cwd: &Path, words: &[&str]) -> Asked {
    let plan = SchedulePlan {
        args: parsed(words),
        language: Language::Chinese,
        cwd: cwd.to_path_buf(),
        zone: TimeZone::get("Asia/Tokyo").expect("认得东京"),
    };
    let (connection, token) = miyu_ipc::connect(&home.root).await.expect("连得上");
    let tape = Tape::default();
    let (mut out, mut err) = (tape.pen(false), tape.pen(true));
    let code = within(
        "跑完",
        schedule_on(connection, &token, &plan, &mut out, &mut err),
    )
    .await;
    Asked {
        code,
        out: tape.text(|err| !err),
        err: tape.text(|err| err),
        screen: tape.text(|_| true),
    }
}

/// `list --format json` 列出来的。
async fn listed(home: &Home, cwd: &Path) -> Vec<Value> {
    let listed = run(home, cwd, &["list", "--format", "json"]).await;
    assert_eq!(listed.code, 0, "{}", listed.err);
    serde_json::from_str(listed.out.trim()).expect("是 JSON")
}

#[tokio::test]
async fn add_list_pause_resume_and_remove() {
    let home = Home::new(Arc::new(Script::new([])));
    let cwd = home.outside();

    let none = run(&home, &cwd, &[]).await;
    assert_eq!((none.code, none.out.as_str()), (0, "没有定时任务\n"));
    let made = run(
        &home,
        &cwd,
        &["add", "--every", "10m", "--no-persona", "看一眼", "服务器"],
    )
    .await;
    assert_eq!(made.code, 0, "{}", made.err);
    // 预设、目录没写的核心照默认定下来，印的是定下来的。
    assert!(
        made.out.starts_with("已添加 t1  every 10m  下一次 ")
            && made.out.contains("  新会话 无人格 预设 ")
            && made.out.ends_with("  「看一眼 服务器」\n"),
        "{}",
        made.out
    );
    let shown = run(&home, &cwd, &["list"]).await;
    assert_eq!(
        (shown.code, shown.out.as_str()),
        (0, made.out.trim_start_matches("已添加 ")),
        "列出来的和建好时印的一样"
    );
    let items = listed(&home, &cwd).await;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["when"]["every"], 600);
    assert_eq!(items[0]["prompt"], "看一眼 服务器");

    let paused = run(&home, &cwd, &["pause", "t1"]).await;
    assert!(
        paused.code == 0
            && paused
                .out
                .starts_with("已暂停 t1  every 10m  停着  新会话 无人格 ")
            && !paused.out.contains("下一次"),
        "停着的没有下一次：{}{}",
        paused.out,
        paused.err
    );
    let resumed = run(&home, &cwd, &["resume", "t1"]).await;
    assert!(
        resumed.code == 0 && resumed.out.starts_with("已恢复 t1  every 10m  下一次 "),
        "{}{}",
        resumed.out,
        resumed.err
    );
    let removed = run(&home, &cwd, &["rm", "t1"]).await;
    assert_eq!(
        (removed.code, removed.out.as_str()),
        (0, "已删除 t1\n"),
        "{}",
        removed.err
    );
    assert!(listed(&home, &cwd).await.is_empty());
}

#[tokio::test]
async fn a_cron_takes_this_machines_zone_or_the_written_one() {
    let home = Home::new(Arc::new(Script::new([])));
    let cwd = home.outside();
    let local = run(
        &home,
        &cwd,
        &["add", "--cron", "0 8 * * *", "--no-persona", "早报"],
    )
    .await;
    assert_eq!(local.code, 0, "{}", local.err);
    assert!(
        local.out.contains("cron 0 8 * * *（Asia/Tokyo）"),
        "{}",
        local.out
    );
    let written = run(
        &home,
        &cwd,
        &[
            "add",
            "--cron",
            "0 8 * * *",
            "--zone",
            "America/New_York",
            "--once",
            "--no-persona",
            "早报",
        ],
    )
    .await;
    assert_eq!(written.code, 0, "{}", written.err);
    let items = listed(&home, &cwd).await;
    let zones: Vec<&str> = items
        .iter()
        .filter_map(|item| item["when"]["zone"].as_str())
        .collect();
    assert_eq!(zones.len(), 2, "{items:?}");
    assert!(zones.contains(&"Asia/Tokyo") && zones.contains(&"America/New_York"));
    let once = items
        .iter()
        .find(|item| item["when"]["zone"] == "America/New_York")
        .expect("有");
    assert_eq!(once["when"]["recurring"], false);
}

#[tokio::test]
async fn a_relative_folder_becomes_its_real_place() {
    let home = Home::new(Arc::new(Script::new([])));
    let cwd = home.outside();
    std::fs::create_dir_all(cwd.join("proj")).expect("建得了");
    let made = run(
        &home,
        &cwd,
        &[
            "add",
            "--every",
            "1h",
            "--no-persona",
            "--cwd",
            "proj",
            "看",
        ],
    )
    .await;
    assert_eq!(made.code, 0, "{}", made.err);
    let real = std::fs::canonicalize(cwd.join("proj")).expect("在");
    assert_eq!(
        listed(&home, &cwd).await[0]["target"]["new"]["cwd"],
        real.to_string_lossy().as_ref()
    );
}

#[tokio::test]
async fn refusals_say_what_was_wrong() {
    let home = Home::new(Arc::new(Script::new([])));
    let cwd = home.outside();
    let short = run(
        &home,
        &cwd,
        &["add", "--every", "30s", "--no-persona", "看"],
    )
    .await;
    assert_eq!(
        (short.code, short.err.as_str()),
        (1, "miyu: 参数错误：间隔至少 60 秒\n")
    );
    let past = run(
        &home,
        &cwd,
        &["add", "--at", "2020-01-01 08:00", "--no-persona", "看"],
    )
    .await;
    assert_eq!(
        (past.code, past.err.as_str()),
        (1, "miyu: 参数错误：时间已经过去\n")
    );
    let zone = run(
        &home,
        &cwd,
        &["add", "--cron", "0 8 * * *", "--zone", "Mars/Olympus", "看"],
    )
    .await;
    assert_eq!(
        (zone.code, zone.err.as_str()),
        (1, "miyu: 参数错误：未知时区\n")
    );
    let missing = run(&home, &cwd, &["rm", "t99"]).await;
    assert_eq!(missing.code, 1);
    assert!(!missing.err.is_empty() && missing.out.is_empty());
    let unread = run(&home, &cwd, &["add", "--every", "soon", "看"]).await;
    assert_eq!(unread.code, 2);
    assert!(
        unread.err == "miyu: 无法识别间隔 'soon'\n提示：写秒数，或 90s、10m、2h、1d\n",
        "{}",
        unread.err
    );
    assert!(listed(&home, &cwd).await.is_empty(), "拒了的都没建");
}
