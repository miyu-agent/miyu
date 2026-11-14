//! 到点交进来的话的标签（施工 J-1 上）；人生那一对标签和出厂的推演方法（施工 LF-2）。

use std::fs;
use std::path::Path;

use super::super::ResourceRoot;
use crate::test_support::Scratch;

/// 源码树里的资源目录：出厂的那一份。
fn repo() -> ResourceRoot {
    ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
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
            life_open: None,
            life_close: None,
        })
    );
}

/// 人生那一对（施工 LF-2）：两份都在才有，缺一份的只当人生的没有，别的四份照旧；出厂的两份和推演方法读得出原文。
#[test]
fn the_life_pair_needs_both_files_and_the_method_reads_as_shipped() {
    let scratch = Scratch::new();
    for (package, name) in [
        ("schedule", "message-open.txt"),
        ("schedule", "message-close.txt"),
        ("loop", "message-open.txt"),
        ("loop", "message-close.txt"),
        ("life", "message-open.txt"),
    ] {
        let dir = scratch.path().join("software").join(package).join("common");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(name), "<x {id}>\n").unwrap();
    }
    let root = ResourceRoot::at(scratch.path());
    let texts = root.schedule_texts().expect("前四份都在");
    assert_eq!((texts.life_open, texts.life_close), (None, None), "缺一份");
    assert!(root.life_method().is_err(), "没有推演方法的报错");
    assert!(root.loop_autonomous().is_err(), "没有自主循环那句的报错");

    let texts = repo().schedule_texts().expect("出厂的都在");
    assert_eq!(
        texts.life_open.as_deref(),
        Some(include_str!(
            "../../../../../resources/software/life/common/message-open.txt"
        ))
    );
    assert_eq!(
        texts.life_close.as_deref(),
        Some(include_str!(
            "../../../../../resources/software/life/common/message-close.txt"
        ))
    );
    assert_eq!(
        repo().life_method().unwrap(),
        include_str!("../../../../../resources/software/life/method.txt")
    );
    assert_eq!(
        repo().loop_autonomous().unwrap(),
        include_str!("../../../../../resources/software/loop/autonomous.txt")
    );
}
