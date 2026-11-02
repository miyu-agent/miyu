//! 两个暂存：核心起来只清自己的 `tmp/unpack/`；`var/tmp/` 清掉放了满七天没碰的，目录看整棵里最近改过的那一刻。

use std::fs;
use std::time::{Duration, SystemTime};

use super::*;
use crate::test_support::Scratch;

/// 一天。
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

/// 把文件 `path` 的修改时刻改成 `at`。
fn touch(path: &Path, at: SystemTime) {
    fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("打得开")
        .set_modified(at)
        .expect("改得了时刻");
}

#[test]
fn starting_clears_only_the_cores_own_short_lived_scratch() {
    let scratch = Scratch::new();
    let root = DataRoot::at(scratch.path().to_path_buf());
    fs::create_dir_all(root.unpack_staging().join("half")).expect("建得了");
    let package = scratch.path().join("tmp").join("web");
    fs::create_dir_all(&package).expect("建得了");
    root.clear_own_tmp().expect("清得了");
    assert!(!root.unpack_staging().exists());
    assert!(package.is_dir(), "包的 tmp/<编号>/ 在拉起它之前才清");
    root.clear_own_tmp().expect("没有也不算错");
}

#[test]
fn var_tmp_keeps_what_was_touched_within_seven_days() {
    let scratch = Scratch::new();
    let root = DataRoot::at(scratch.path().to_path_buf());
    let var_tmp = root.var_tmp();
    assert_eq!(var_tmp, scratch.path().join("var").join("tmp"));
    fs::create_dir_all(var_tmp.join("tree").join("deep")).expect("建得了");
    fs::write(var_tmp.join("tree").join("deep").join("f"), b"x").expect("写得进");
    fs::write(var_tmp.join("loose"), b"x").expect("写得进");
    let now = SystemTime::now();
    // 刚碰过的都留着。
    let swept = root.sweep_var_tmp(now).expect("读得了");
    assert!(swept.removed.is_empty() && swept.failed.is_empty());
    // 六天前碰过的照样留着。
    let swept = root.sweep_var_tmp(now + 6 * DAY).expect("读得了");
    assert!(swept.removed.is_empty());
    // 满七天没碰的清掉，目录整棵清。
    let swept = root.sweep_var_tmp(now + 7 * DAY + DAY / 2).expect("读得了");
    assert_eq!(swept.removed.len(), 2, "{swept:?}");
    assert!(swept.failed.is_empty());
    assert_eq!(fs::read_dir(&var_tmp).expect("还在").count(), 0);
    assert!(var_tmp.is_dir(), "var/tmp/ 本身留着");
}

#[test]
fn a_directory_counts_as_touched_when_anything_inside_was() {
    let scratch = Scratch::new();
    let root = DataRoot::at(scratch.path().to_path_buf());
    let deep = root.var_tmp().join("job").join("deep");
    fs::create_dir_all(&deep).expect("建得了");
    let now = SystemTime::now();
    let inner = deep.join("f");
    fs::write(&inner, b"x").expect("写得进");
    // 里面的一份是后来才碰的：整棵算那时碰过。
    touch(&inner, now + 5 * DAY);
    let swept = root.sweep_var_tmp(now + 8 * DAY).expect("读得了");
    assert!(swept.removed.is_empty(), "{swept:?}");
    assert!(inner.is_file());
    let swept = root.sweep_var_tmp(now + 13 * DAY).expect("读得了");
    assert_eq!(swept.removed, [root.var_tmp().join("job")]);
}

#[test]
fn no_var_tmp_is_not_an_error() {
    let scratch = Scratch::new();
    let root = DataRoot::at(scratch.path().to_path_buf());
    let swept = root.sweep_var_tmp(SystemTime::now()).expect("没有不算错");
    assert!(swept.removed.is_empty());
}

#[cfg(unix)]
#[test]
fn a_link_is_removed_without_following_it() {
    let scratch = Scratch::new();
    let elsewhere = Scratch::new();
    let root = DataRoot::at(scratch.path().to_path_buf());
    fs::create_dir_all(root.var_tmp()).expect("建得了");
    let kept = elsewhere.path().join("kept");
    fs::write(&kept, b"x").expect("写得进");
    std::os::unix::fs::symlink(elsewhere.path(), root.var_tmp().join("link")).expect("建得了");
    let swept = root
        .sweep_var_tmp(SystemTime::now() + 8 * DAY)
        .expect("读得了");
    assert_eq!(swept.removed, [root.var_tmp().join("link")]);
    assert!(kept.is_file(), "链接那头一点没动");
}

/// `var/`、`var/tmp/` 是指到数据根外面的链接（施工 FH-3下，core1 审 FH-3上 时提的）：整趟不清、交回错，外面的一样都不删。
#[test]
fn a_linked_var_or_var_tmp_is_not_swept() {
    for linked in ["var", "var/tmp"] {
        let scratch = Scratch::new();
        let elsewhere = Scratch::bare();
        fs::create_dir_all(elsewhere.path()).expect("建得了");
        let outside = elsewhere.path().join("old");
        fs::write(&outside, b"x").expect("写得进");
        let root = DataRoot::at(scratch.path().to_path_buf());
        let at = scratch.path().join(linked);
        fs::create_dir_all(at.parent().expect("有上一级")).expect("建得了");
        if !crate::test_support::link(elsewhere.path(), &at) {
            return;
        }
        let swept = root.sweep_var_tmp(SystemTime::now() + 30 * DAY);
        assert!(swept.is_err(), "{linked}：是链接的整趟不清");
        assert!(outside.is_file(), "{linked}：链接那头一样都没删");
    }
}

/// `tmp/` 是指到外面的链接：核心起来清 `tmp/unpack/` 不顺着它删到外面去。
#[test]
fn a_linked_tmp_is_not_cleared_through() {
    let scratch = Scratch::new();
    let elsewhere = Scratch::bare();
    fs::create_dir_all(elsewhere.path().join("unpack")).expect("建得了");
    let outside = elsewhere.path().join("unpack/kept");
    fs::write(&outside, b"x").expect("写得进");
    let root = DataRoot::at(scratch.path().to_path_buf());
    if !crate::test_support::link(elsewhere.path(), &scratch.path().join("tmp")) {
        return;
    }
    assert!(root.clear_own_tmp().is_err(), "是链接的不清");
    assert!(outside.is_file(), "链接那头一样都没删");
}
