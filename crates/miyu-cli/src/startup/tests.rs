//! 连上核心以前出的错照界面语言说（施工 EN-1）：英文界面不印中文原话；没有说法的照原话。

use std::path::PathBuf;

use miyu_ipc::ConnectError;
use miyu_store::root::{PrepareError, RootError};

use super::{Early, spoken};

/// 照英文说（测试的进程里界面语言照 `LANG`，这里不改环境，直接照说法取英文的字）。
fn english(error: &dyn Early) -> Option<String> {
    let resources = miyu_store::resources::ResourceRoot::at(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"),
    );
    let words = miyu_store::human::Human::load(&resources, "en").expect("读得出");
    error.said().and_then(|said| words.say(&said))
}

#[test]
fn data_root_and_connect_errors_are_said_in_english() {
    let relative = RootError::RelativeMiyuHome(PathBuf::from("rel/path"));
    assert_eq!(
        english(&relative).as_deref(),
        Some("MIYU_HOME must be an absolute path, got rel/path")
    );
    assert_eq!(
        english(&ConnectError::NotRunning).as_deref(),
        Some("The core isn't running")
    );
    let old = PrepareError::OldLayout(PathBuf::from("/x"));
    assert!(
        english(&old).is_some_and(|said| said.starts_with("/x is an old Miyu data root")),
        "{:?}",
        english(&old)
    );
}

#[test]
fn errors_without_words_keep_their_own() {
    let io = PrepareError::Io(std::io::Error::other("disk full"));
    assert_eq!(io.said(), None);
    assert_eq!(spoken(&io), "disk full");
}
