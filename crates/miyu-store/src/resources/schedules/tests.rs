//! 到点交进来的话的标签（施工 J-1 上），光打 `/loop` 交进去的那句（施工 LO-1）。

use std::fs;
use std::path::Path;

use super::super::ResourceRoot;
use crate::test_support::Scratch;

/// 源码树里的资源目录：出厂的那一份。
fn repo() -> ResourceRoot {
    ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
}

#[test]
fn the_autonomous_loop_line_reads_as_shipped() {
    let scratch = Scratch::new();
    let root = ResourceRoot::at(scratch.path());
    assert!(root.loop_autonomous().is_err(), "没有自主循环那句的报错");
    assert_eq!(
        repo().loop_autonomous().unwrap(),
        include_str!("../../../../../resources/software/loop/autonomous.txt")
    );
    // 施工 WK-2：说「下一轮」，不说醒。
    assert_eq!(
        repo().loop_autonomous().unwrap().trim_end(),
        "No new instructions. Continue on your own judgment, then schedule the next round or stop."
    );
}

/// 到点交进来的话的标签（施工 J-1 上）：四份都在才读得出，原样；缺一份的当没有。
#[test]
fn schedule_texts_need_all_four_files() {
    let scratch = Scratch::new();
    let root = ResourceRoot::at(scratch.path());
    assert_eq!(root.schedule_texts(), None, "一份都没有");
    for (package, name, text) in [
        ("schedule", "message-open.txt", "<task {id}>\n"),
        ("schedule", "message-close.txt", "</task>\n"),
        ("loop", "message-open.txt", "<wakeup {id}>\n"),
    ] {
        let dir = scratch.path().join("software").join(package).join("common");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(name), text).unwrap();
    }
    assert_eq!(root.schedule_texts(), None, "缺一份");
    fs::write(
        scratch
            .path()
            .join("software/loop/common/message-close.txt"),
        "again?\n</wakeup>\n",
    )
    .unwrap();
    assert_eq!(
        root.schedule_texts(),
        Some(miyu_policy::ScheduleTexts {
            schedule_open: "<task {id}>\n".to_string(),
            schedule_close: "</task>\n".to_string(),
            loop_open: "<wakeup {id}>\n".to_string(),
            loop_close: "again?\n</wakeup>\n".to_string(),
        })
    );
}
