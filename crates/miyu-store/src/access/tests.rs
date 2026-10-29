//! 访问密码（施工 W-8，SU-6 上只剩一份）：没有的是空的；写的读回来一字不差、0600；坏了的说是坏了；argon2id 的参数照写的、
//! 对的验得过、错的验不过；`Debug` 不印哈希。

use std::fs;

use super::*;
use crate::test_support::Scratch;

fn set(password: &str) -> Access {
    Access {
        password: hash(password).expect("算得出"),
        changed: Some(Timestamp::parse("2026-10-04T06:00:00.000Z").expect("合写法")),
        ..Access::default()
    }
}

#[test]
fn a_missing_file_is_unset_and_a_written_one_reads_back() {
    let temp = Scratch::new();
    let path = temp.path().join(FILE);
    let empty = read(&path).expect("没有的是空的");
    assert_eq!(empty.access, Access::default());
    assert_eq!(empty.version, None);
    let access = set("correct horse");
    write(&path, &access, None).expect("写得进");
    let again = read(&path).expect("读得回");
    assert_eq!(again.access, access);
    let text = fs::read_to_string(&path).expect("在");
    assert!(
        text.starts_with("{\"version\":1,\"password\":\"$argon2id$v=19$m=19456,t=2,p=1$"),
        "{text}"
    );
    assert!(
        text.ends_with("\"changed\":\"2026-10-04T06:00:00.000Z\"}\n"),
        "{text}"
    );
    // 上一次读到以后被人改过的，不盖。
    let changed = set("other horse");
    assert!(write(&path, &changed, None).is_err(), "写之前文件已经在了");
    write(&path, &changed, again.version.as_deref()).expect("照读到的版本写");
    assert_eq!(read(&path).expect("读得回").access, changed);
}

#[test]
fn a_broken_file_says_so() {
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).expect("建得出");
    let path = temp.path().join(FILE);
    for bad in [
        "not json",
        "{\"version\":2}",
        "{\"version\":1,\"accounts\":[]}",
        "{\"version\":1,\"password\":1}",
    ] {
        fs::write(&path, bad).expect("写得进");
        assert!(read(&path).is_err(), "{bad}");
    }
}

#[cfg(unix)]
#[test]
fn the_file_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let temp = Scratch::new();
    let path = temp.path().join(FILE);
    write(&path, &set("correct horse"), None).expect("写得进");
    let mode = fs::metadata(&path).expect("在").permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn passwords_hash_with_argon2id_and_verify() {
    let phc = hash("correct horse").expect("算得出");
    assert!(phc.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"), "{phc}");
    assert_ne!(hash("correct horse").expect("算得出"), phc, "盐每次不一样");
    assert!(verify(&phc, "correct horse"));
    assert!(!verify(&phc, "correct horsE"));
    assert!(!verify(&phc, ""));
    assert!(!verify("not a hash", "correct horse"), "读不懂的哈希验不过");
    let shown = format!("{:?}", set("correct horse"));
    assert!(!shown.contains("argon2"), "{shown}");
}
