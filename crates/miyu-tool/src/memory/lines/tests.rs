//! 一行的写法（施工 E-4 下）：有群名的、群没群名的、私聊、别的形状、本机的逐字节；群名里的换行、引号、尖括号照整行的规矩
//! 转义一次；作废的照另一份。名字、群号、群名都是编的。

use std::path::{Path, PathBuf};

use super::*;

/// 出厂的资源目录。
fn resources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

fn lines() -> MemoryLines {
    MemoryLines::load(&resources(), "memory").expect("出厂的读得出")
}

fn venue(text: &str) -> VenueId {
    VenueId::parse(text).expect("合写法")
}

fn said(venue: Option<&VenueId>, place: Option<&str>, retired: bool) -> String {
    lines().say(&Line {
        id: "m12",
        class: "episode",
        date: "2026-10-21",
        text: "一起去海边拍了日落",
        venue,
        place,
        retired,
    })
}

#[test]
fn a_group_with_its_name_is_written_with_the_name() {
    let group = venue("qq:group:900001");
    assert_eq!(
        said(Some(&group), Some("周末摄影"), false),
        "m12 episode 2026-10-21 in QQ group 「周末摄影」: 一起去海边拍了日落\n"
    );
}

#[test]
fn a_group_without_a_name_says_it_was_a_group() {
    let group = venue("qq:group:900001");
    assert_eq!(
        said(Some(&group), None, false),
        "m12 episode 2026-10-21 in a QQ group: 一起去海边拍了日落\n"
    );
}

#[test]
fn a_private_chat_says_so() {
    let private = venue("qq:private:10086");
    assert_eq!(
        said(Some(&private), None, false),
        "m12 episode 2026-10-21 in a QQ private chat: 一起去海边拍了日落\n"
    );
}

/// 场所编号多一段机器人账号的（施工 O-63，`qq:<机器人>:group:<群号>`）：照右边认，最后一段是号、倒数第二段是种类，中间那一段
/// 不看；机器人账号正好叫 `group` 的也不认错。
#[test]
fn a_venue_with_a_bot_segment_is_read_from_the_right() {
    let group = venue("qq:20001:group:900001");
    assert_eq!(
        said(Some(&group), None, false),
        "m12 episode 2026-10-21 in a QQ group: 一起去海边拍了日落\n"
    );
    assert_eq!(
        said(Some(&group), Some("周末摄影"), false),
        "m12 episode 2026-10-21 in QQ group 「周末摄影」: 一起去海边拍了日落\n"
    );
    let private = venue("qq:20001:private:10086");
    assert_eq!(
        said(Some(&private), None, false),
        "m12 episode 2026-10-21 in a QQ private chat: 一起去海边拍了日落\n"
    );
    let tricky = venue("qq:group:private:10086");
    assert_eq!(
        said(Some(&tricky), None, false),
        "m12 episode 2026-10-21 in a QQ private chat: 一起去海边拍了日落\n",
        "机器人账号叫 group 的私聊"
    );
}

#[test]
fn another_shape_only_names_the_platform() {
    let other = venue("matrix:room-7");
    assert_eq!(
        said(Some(&other), None, false),
        "m12 episode 2026-10-21 on matrix: 一起去海边拍了日落\n"
    );
}

#[test]
fn here_adds_nothing() {
    assert_eq!(
        said(None, None, false),
        "m12 episode 2026-10-21: 一起去海边拍了日落\n"
    );
    assert_eq!(
        said(None, Some("周末摄影"), false),
        "m12 episode 2026-10-21: 一起去海边拍了日落\n",
        "没有场所的群名不算"
    );
}

#[test]
fn a_retired_one_keeps_the_place() {
    let group = venue("qq:group:900001");
    assert_eq!(
        said(Some(&group), Some("周末摄影"), true),
        "m12 episode 2026-10-21 in QQ group 「周末摄影」 (retired): 一起去海边拍了日落\n"
    );
}

#[test]
fn a_name_is_escaped_once_with_the_line() {
    let group = venue("qq:group:900001");
    assert_eq!(
        said(Some(&group), Some("周末\n\"摄影\"<b>&"), false),
        "m12 episode 2026-10-21 in QQ group 「周末\\n\\u0022摄影\\u0022\\u003cb\\u003e\\u0026」: 一起去海边拍了日落\n"
    );
}
