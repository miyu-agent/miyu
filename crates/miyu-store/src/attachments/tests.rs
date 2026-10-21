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
    assert_eq!(
        clean(&"字".repeat(200)).len(),
        NAME_BYTES,
        "照字节截（鲁棒性 B-8）"
    );
}

/// 截名字照字节、截在字的边界上，扩展名留着；长得不像扩展名的整个截（鲁棒性 B-8）。
#[test]
fn long_names_are_clipped_by_bytes_keeping_the_extension() {
    let report = clean(&format!("{}.pdf", "字".repeat(200)));
    assert!(
        report.len() <= NAME_BYTES && report.ends_with(".pdf"),
        "{report}"
    );
    assert_eq!(report.len(), 3 * 58 + ".pdf".len(), "截在字的边界上");
    let shifted = clean(&format!("a{}", "字".repeat(100)));
    assert_eq!(shifted.len(), 1 + 3 * 59, "一个字不切开：{shifted}");
    let odd = clean(&format!("{}.{}", "字".repeat(100), "x".repeat(20)));
    assert_eq!(odd.len(), NAME_BYTES, "扩展名太长的整个截：{odd}");
    assert!(!odd.contains('.'), "{odd}");
    assert_eq!(
        with_extension("paste", &format!("image/{}", "x".repeat(16))),
        "paste",
        "太长的子类型不补"
    );
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
        .filter(|name| name.starts_with('.'))
        .collect();
    assert!(leftovers.is_empty(), "临时文件不留：{leftovers:?}");
}

/// 鲁棒性（2026-10-22）：临时文件的名字是固定的 `.{名字}.miyu-tmp`（`keep` 里交给 `create_temp` 的闭包每次都一样），上一次写到一半
/// 崩了留下一个，`create_temp` 重试几十次都撞它，`?` 直接报错，不退到 ` (2)`：这个会话以后这个名字的附件永远存不进。
/// 期望：照样存得进。修之前这条是红的。
#[test]
fn a_leftover_temp_file_does_not_block_the_name_forever() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("attachments");
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::write(dir.join(".a.txt.miyu-tmp"), b"half").expect("写得进");
    let kept = keep(&dir, "a.txt", "text/plain", b"hello");
    assert!(kept.is_ok(), "残留一个临时文件以后存不进：{kept:?}");
}

/// 鲁棒性（2026-10-22）：名字上限 `NAME_MAX` 照字符数算（120），120 个汉字是 360 字节，再包上临时名的点和后缀，超过常见文件系统
/// 一个名字 255 字节的上限：长中文名的附件一律存不进。期望：存得进（名字照字节截）。修之前这条在 Linux 上是红的。
#[test]
fn a_long_chinese_name_still_fits_the_file_system() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("attachments");
    let kept = keep(
        &dir,
        &format!("{}.txt", "字".repeat(200)),
        "text/plain",
        b"hello",
    );
    assert!(kept.is_ok(), "长中文名存不进：{kept:?}");
}
