//! 附件落成文件（施工 3-9 六补）：名字怎么清、补扩展名、撞名接序号、同一份内容复用、交回真实位置、目录是新建的也行。

use super::*;
use crate::test_support::Scratch;

#[test]
fn names_are_cleaned_for_every_platform() {
    assert_eq!(clean("截图 2026-10-16.png"), "截图 2026-10-16.png");
    assert_eq!(
        clean("../../etc/passwd"),
        "_.._etc_passwd",
        "路径分隔符换掉，两头的点去掉"
    );
    assert_eq!(clean("a:b*c?d\"e<f>g|h\\i"), "a_b_c_d_e_f_g_h_i");
    assert_eq!(clean("tab\there\n"), "tab_here_");
    assert_eq!(clean("  名字.  "), "名字", "两头的空格和点去掉");
    assert_eq!(clean("..."), "attachment", "什么都不剩的");
    assert_eq!(clean("CON.png"), "_CON.png", "Windows 的保留名");
    assert_eq!(clean("con"), "_con");
    assert_eq!(clean("console.png"), "console.png", "只是开头像的不算");
    assert_eq!(clean(&"字".repeat(200)).chars().count(), NAME_MAX);
}

#[test]
fn a_missing_extension_comes_from_the_media_type() {
    assert_eq!(with_extension("粘贴的图片", "image/png"), "粘贴的图片.png");
    assert_eq!(
        with_extension("a.jpg", "image/png"),
        "a.jpg",
        "有扩展名的不动"
    );
    assert_eq!(with_extension("a", "image/svg+xml"), "a", "认不出的照原样");
    assert_eq!(
        with_extension(".hidden", "text/plain"),
        ".hidden.plain",
        "开头的点不算扩展名"
    );
}

#[test]
fn a_file_is_kept_and_a_clash_gets_a_number() {
    let scratch = Scratch::new();
    let dir = scratch
        .path()
        .join("sessions")
        .join("s")
        .join("attachments");
    let first = keep(&dir, "截图.png", "image/png", b"one").unwrap();
    assert_eq!(
        first,
        fs::canonicalize(dir.join("截图.png")).unwrap(),
        "交回真实位置"
    );
    assert_eq!(fs::read(&first).unwrap(), b"one");
    let again = keep(&dir, "截图.png", "image/png", b"one").unwrap();
    assert_eq!(again, first, "同一份内容同一个名字的用原来那个");
    let second = keep(&dir, "截图.png", "image/png", b"two").unwrap();
    assert_eq!(second.file_name().unwrap(), "截图 (2).png");
    assert_eq!(fs::read(&second).unwrap(), b"two");
    let third = keep(&dir, "截图.png", "image/png", b"three").unwrap();
    assert_eq!(third.file_name().unwrap(), "截图 (3).png");
    let bare = keep(&dir, "paste", "image/jpeg", b"jpg").unwrap();
    assert_eq!(bare.file_name().unwrap(), "paste.jpeg");
    let leftovers: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.contains("miyu-tmp"))
        .collect();
    assert!(leftovers.is_empty(), "临时文件不留：{leftovers:?}");
}
