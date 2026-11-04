//! 附件文件夹里起名字、写链接（施工 SE-1 中下下）：名字收拾成三个平台都收的、撞名的接序号、同一个 blob 一份、链接里的怪字写成
//! `%XX`、取不到的记下来。

use miyu_kernel::id::ContentHash;

use super::{Files, clean, extension};

fn blob(n: u8) -> ContentHash {
    ContentHash::of(&[n])
}

#[test]
fn names_are_safe_on_every_platform() {
    for (name, wanted) in [
        ("shot.png", "shot.png"),
        ("/home/alice/Pictures/截图 1.png", "截图 1.png"),
        ("C:\\Users\\a\\b.txt", "b.txt"),
        ("a<b>:c\"d|e?f*.md", "a_b__c_d_e_f_.md"),
        ("  .hidden.  ", "hidden"),
        ("CON.txt", "_CON.txt"),
        ("lpt1", "_lpt1"),
        ("COMMON.txt", "COMMON.txt"),
        ("tab\there.png", "tab_here.png"),
        ("....", ""),
    ] {
        assert_eq!(clean(name), wanted, "{name:?}");
    }
    let long = format!("{}.png", "长".repeat(200));
    let cleaned = clean(&long);
    assert_eq!(cleaned.chars().count(), 80, "{cleaned}");
    assert!(cleaned.ends_with("长.png"), "扩展名留着：{cleaned}");
    assert_eq!(extension("image/png"), "png");
    assert_eq!(extension("image/svg+xml"), "svg");
    assert_eq!(extension("text/plain; charset=utf-8"), "plain");
    assert_eq!(extension("application/vnd.openxmlformats"), "bin");
    assert_eq!(extension("nonsense"), "bin");
}

#[test]
fn one_file_per_blob_and_clashes_get_numbers() {
    let mut files = Files::new("my talk_files");
    files.have([blob(1), blob(2), blob(3), blob(4)]);
    let one = files.link(&blob(1), true, None, (Some("a.png"), "image/png"));
    let again = files.link(
        &blob(1),
        true,
        Some("别的".into()),
        (Some("z.png"), "image/png"),
    );
    let clash = files.link(&blob(2), true, None, (Some("A.PNG"), "image/png"));
    let third = files.link(
        &blob(3),
        false,
        Some("说明 [1]".into()),
        (Some("a.png"), "x/y"),
    );
    let bare = files.link(&blob(4), true, None, (None, "image/webp"));
    assert_eq!(one.as_deref(), Some("![a.png](my%20talk_files/a.png)"));
    assert_eq!(
        again.as_deref(),
        Some("![别的](my%20talk_files/a.png)"),
        "同一个 blob 用头一回起的名字"
    );
    assert_eq!(
        clash.as_deref(),
        Some("![A-2.PNG](my%20talk_files/A-2.PNG)"),
        "不分大小写算撞"
    );
    assert_eq!(
        third.as_deref(),
        Some("[说明 \\[1\\]](my%20talk_files/a-3.png)"),
        "附件写成链接，说明里的方括号垫反斜杠"
    );
    let hex: String = blob(4).hex().chars().take(8).collect();
    assert_eq!(
        bare,
        Some(format!(
            "![image-{hex}.webp](my%20talk_files/image-{hex}.webp)"
        ))
    );
    assert_eq!(files.named().len(), 4);
    assert!(files.missing().is_empty());
}

#[test]
fn odd_characters_in_links_are_escaped_and_missing_blobs_are_noted() {
    let mut files = Files::new("a(b)#c_files");
    files.have([blob(1)]);
    let link = files.link(
        &blob(1),
        false,
        None,
        (Some("100% [x] `y`.txt"), "text/plain"),
    );
    assert_eq!(
        link.as_deref(),
        Some("[100% \\[x\\] `y`.txt](a%28b%29%23c_files/100%25%20%5Bx%5D%20%60y%60.txt)")
    );
    assert_eq!(files.link(&blob(9), true, None, (None, "image/png")), None);
    assert_eq!(
        files.missing().into_iter().collect::<Vec<_>>(),
        vec![blob(9)]
    );
    assert_eq!(files.named().len(), 1, "取不到的不起名字");
}
