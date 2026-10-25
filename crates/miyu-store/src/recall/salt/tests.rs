use std::fs;

use miyu_kernel::id::{AccountId, SessionId};

use super::{FILE, LEN, LogSalt};
use crate::env::{Env, Platform};
use crate::recall::Room;
use crate::root::DataRoot;
use crate::test_support::Scratch;

/// 临时目录下的 `data`，当数据根，建好了骨架。
fn root_in(scratch: &Scratch) -> DataRoot {
    let root = DataRoot::locate(&Env {
        platform: Platform::current(),
        miyu_home: Some(scratch.path().join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    })
    .unwrap();
    root.prepare().unwrap();
    root
}

fn account(name: &str) -> AccountId {
    AccountId::parse(name).unwrap()
}

#[test]
fn the_same_room_gets_the_same_short_id_and_others_differ() {
    let salt = LogSalt::fresh();
    let mine = Room::persona(&account("alice"), "miyu");
    let id = salt.room_id(&mine);
    assert_eq!(id.len(), 8, "8 位：{id}");
    assert!(
        id.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    );
    assert_eq!(salt.room_id(&mine.clone()), id);
    assert_ne!(salt.room_id(&Room::persona(&account("bob"), "miyu")), id);
    assert_ne!(salt.room_id(&Room::persona(&account("alice"), "luna")), id);
    let session = SessionId::parse("0192f3a0-0000-7000-8000-000000000001").unwrap();
    let own = Room::session(&account("alice"), &session);
    assert_ne!(salt.room_id(&own), id);
    assert_eq!((mine.kind(), own.kind()), ("persona", "session"));
}

#[test]
fn a_different_salt_gives_a_different_short_id() {
    let room = Room::persona(&account("alice"), "miyu");
    assert_ne!(
        LogSalt::fresh().room_id(&room),
        LogSalt::fresh().room_id(&room)
    );
}

#[test]
fn the_short_id_does_not_show_the_account_or_the_persona() {
    let salt = LogSalt::fresh();
    let id = salt.room_id(&Room::persona(&account("alice"), "miyu"));
    assert!(!id.contains("alice") && !id.contains("miyu"));
    assert_eq!(format!("{salt:?}"), "LogSalt(..)");
}

#[test]
fn a_missing_salt_is_made_and_kept_then_read_back() {
    let scratch = Scratch::new();
    let root = root_in(&scratch);
    let path = root.state().join(FILE);
    assert!(!path.exists());
    let first = LogSalt::load(&root).unwrap();
    assert_eq!(fs::read(&path).unwrap().len(), LEN);
    let again = LogSalt::load(&root).unwrap();
    assert_eq!(first, again);
    let room = Room::persona(&account("alice"), "miyu");
    assert_eq!(first.room_id(&room), again.room_id(&room));
    // 不留临时文件。
    let left: Vec<_> = fs::read_dir(root.state())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(left.is_empty());
}

#[test]
fn a_salt_of_the_wrong_length_is_replaced() {
    let scratch = Scratch::new();
    let root = root_in(&scratch);
    let path = root.state().join(FILE);
    fs::write(&path, b"short").unwrap();
    let salt = LogSalt::load(&root).unwrap();
    let kept = fs::read(&path).unwrap();
    assert_eq!(kept.len(), LEN);
    assert_eq!(LogSalt::load(&root).unwrap(), salt);
}

#[cfg(unix)]
#[test]
fn only_the_owner_reads_the_salt() {
    use std::os::unix::fs::PermissionsExt;

    let scratch = Scratch::new();
    let root = root_in(&scratch);
    LogSalt::load(&root).unwrap();
    let mode = fs::metadata(root.state().join(FILE))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}
