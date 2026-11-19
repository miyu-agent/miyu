//! 连不上核心照这台机器的语言说（施工 EN-2）。

use std::path::Path;

use miyu_ipc::{ConnectError, StartError};
use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;

use super::{language, spoken};

fn words(language: &str) -> Human {
    let resources = ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    Human::load(&resources, language).expect("读得出")
}

#[test]
fn an_unreachable_core_is_said_in_the_machine_language() {
    let english = words("en");
    let not_running = ConnectError::NotRunning;
    assert_eq!(
        spoken(not_running.said(), &not_running, Some(&english)),
        "The core isn't running"
    );
    let log = StartError::Refused {
        code: Some("log_failed".to_string()),
        reason: "File exists (os error 17)".to_string(),
    };
    assert_eq!(
        spoken(log.said(), &log, Some(&english)),
        "The core couldn't start: cannot write its log: File exists (os error 17)"
    );
    assert_eq!(
        spoken(log.said(), &log, None),
        "核心无法启动（log_failed）：File exists (os error 17)",
        "取不到字的照原话"
    );
    assert_eq!(language("ja_JP.UTF-8"), "ja");
    assert_eq!(language("zh_CN.UTF-8"), "zh");
    assert_eq!(language("C"), "en");
}
