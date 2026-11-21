//! 起不来的那一行（施工 EN-2）：每一步都带代码，代码在三种语言里都有字；原因只写会变的那一段。

use std::path::{Path, PathBuf};

use miyu_ipc::{OpenError, Ready, StartError};
use miyu_store::human::Human;
use miyu_store::resources::{ResourceError, ResourceRoot};
use miyu_store::root::{DataRoot, PrepareError};

use super::{CODES, Failure};

/// 头读回来的样子。
fn refused(failure: Failure) -> StartError {
    match Ready::parse(&failure.ready().line()) {
        Ready::Failed { code, reason } => StartError::Refused { code, reason },
        other => panic!("应该起不来：{other:?}"),
    }
}

fn words(language: &str) -> Human {
    let resources = ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    Human::load(&resources, language).expect("读得出")
}

#[test]
fn every_code_has_words_in_three_languages() {
    for language in ["zh", "en", "ja"] {
        let words = words(language);
        for code in CODES {
            let said = refused(Failure::new(code, "/x")).said().expect("有说法");
            assert!(words.say(&said).is_some(), "{language} 没有 start/{code}");
        }
    }
}

#[test]
fn the_reason_is_only_the_part_that_changes() {
    let english = words("en");
    let say = |failure: Failure| {
        english
            .say(&refused(failure).said().expect("有说法"))
            .expect("有字")
    };
    let exists = std::io::Error::from(std::io::ErrorKind::AlreadyExists);
    assert_eq!(
        say(Failure::log(&miyu_log::InstallError::Io(exists))),
        format!(
            "The core couldn't start: cannot write its log: {}",
            std::io::Error::from(std::io::ErrorKind::AlreadyExists)
        )
    );
    assert_eq!(
        say(Failure::resources(&ResourceError::Missing(PathBuf::from(
            "/no"
        )))),
        "The core couldn't start: MIYU_RESOURCES points to /no, which isn't a folder"
    );
    assert_eq!(
        say(Failure::socket(&OpenError::NotPrivate(PathBuf::from(
            "/run/x"
        )))),
        "The core couldn't start: /run/x is not owned by the current user or is accessible to other users, so the socket was not created there"
    );
    let root = DataRoot::at(PathBuf::from("/d"));
    assert!(
        say(Failure::prepare(
            &PrepareError::OldLayout(PathBuf::from("/d")),
            &root
        ))
        .starts_with("/d is an old Miyu data root"),
    );
}
