//! 主人（施工 SU-1）：第一次起来照系统用户名建一个、不设密码；已经有的照旧用他；用户名换成账号的写法，换不成的叫 `me`。

use std::sync::atomic::{AtomicU64, Ordering};

use miyu_store::env::{Env, Platform};

use super::*;

/// 临时的数据根：丢掉时删。
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        Scratch(std::env::temp_dir().join(format!("miyu-core-owner-{}-{n}", std::process::id())))
    }

    fn root(&self) -> DataRoot {
        let root = DataRoot::locate(&Env {
            platform: Platform::current(),
            miyu_home: Some(self.0.join("root").into_os_string()),
            home: None,
            xdg_cache_home: None,
            local_app_data: None,
            miyu_resources: None,
            exe: None,
        })
        .expect("MIYU_HOME 是绝对路径");
        root.prepare().expect("建得了骨架");
        root
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn now() -> Timestamp {
    Timestamp::parse("2026-10-28T10:00:00.000Z").expect("合写法")
}

#[test]
fn the_first_start_creates_the_owner_once() {
    let scratch = Scratch::new();
    let root = scratch.root();
    assert_eq!(owner(&root), None);
    let made = ensure(&root, Some("Shorin"), now()).expect("建得了");
    assert_eq!(made.as_str(), "shorin");
    assert_eq!(owner(&root), Some(made.clone()));
    let read = accounts::read(&root.path().join(accounts::FILE)).expect("读得了");
    let row = read.accounts.of("shorin").expect("在");
    assert!(row.password.is_empty(), "不设访问密码");
    assert_eq!(
        ensure(&root, Some("someone-else"), now()),
        Ok(made),
        "有了就照旧"
    );
}

#[test]
fn unusable_user_names_fall_back() {
    assert_eq!(name_of("Ab.C-1"), "abc-1");
    assert_eq!(name_of("1x"), "u1x");
    for user in [Some("..."), None] {
        let scratch = Scratch::new();
        let made = ensure(&scratch.root(), user, now()).expect("建得了");
        assert_eq!(made.as_str(), "me", "{user:?}");
    }
}
