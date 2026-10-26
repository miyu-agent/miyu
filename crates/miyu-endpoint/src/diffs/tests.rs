//! 差异的几种算不出：不是 UTF-8 的、取不出来的、太大的；两边一样的是空的。效果里写着原文的先用原文、哈希对不上的当没写（施工 V-18）。

use miyu_kernel::event::FileChanged;

use super::*;

#[test]
fn what_cannot_be_diffed_says_why() {
    assert_eq!(unified(b"a\n", &[0xff, 0xfe]), Err(Skipped::Binary));
    let same = unified(b"a\n", b"a\n").expect("是文字");
    assert!(same.lines.is_empty() && same.added == 0 && same.removed == 0);
    let scratch = std::env::temp_dir().join(format!("miyu-diffs-{}", std::process::id()));
    let blobs = Blobs::new(scratch.clone());
    let absent = ContentHash::of(b"never stored");
    assert_eq!(side(&blobs, Some(&absent)), Err(Skipped::Missing));
    assert_eq!(side(&blobs, None), Ok(Vec::new()), "新建的文件的改前当空的");
    let big = vec![b'x'; usize::try_from(MOST_BYTES).unwrap_or(usize::MAX) + 1];
    let stored = blobs.put(&big).expect("存得进");
    assert_eq!(side(&blobs, Some(&stored)), Err(Skipped::TooBig));
    drop(std::fs::remove_dir_all(scratch));
    assert_eq!(Skipped::TooBig.as_str(), "too_big");
    assert_eq!(Skipped::Binary.as_str(), "binary");
    assert_eq!(Skipped::Missing.as_str(), "missing");
}

/// 改了 `/a`：改前、改后的哈希照 `before`、`after` 算，两格原文照写。
fn changed(
    before: Option<&[u8]>,
    after: &[u8],
    texts: (Option<&str>, Option<&str>),
) -> FileChanged {
    FileChanged {
        path: "/a".to_string(),
        before: before.map(ContentHash::of),
        after: ContentHash::of(after),
        before_text: texts.0.map(str::to_string),
        after_text: texts.1.map(str::to_string),
    }
}

#[test]
fn texts_in_the_effect_are_used_before_blobs() {
    let scratch = std::env::temp_dir().join(format!("miyu-diffs-texts-{}", std::process::id()));
    let blobs = Blobs::new(scratch.clone());
    // 两边都写在效果里：一个 blob 都没有也算得出。
    let both = changed(Some(b"a\n"), b"b\n", (Some("a\n"), Some("b\n")));
    assert_eq!(sides(&blobs, &both), Ok((b"a\n".to_vec(), b"b\n".to_vec())));
    // 新建的：改前当空的，改后照原文。
    let created = changed(None, b"n\n", (None, Some("n\n")));
    assert_eq!(sides(&blobs, &created), Ok((Vec::new(), b"n\n".to_vec())));
    // 原文和哈希对不上的当没写：照哈希取 blob，没有就是取不出来。两边各试一次。
    let forged_before = changed(Some(b"a\n"), b"b\n", (Some("x\n"), Some("b\n")));
    assert_eq!(sides(&blobs, &forged_before), Err(Skipped::Missing));
    let forged = changed(Some(b"a\n"), b"b\n", (Some("a\n"), Some("x\n")));
    assert_eq!(sides(&blobs, &forged), Err(Skipped::Missing));
    let stored = blobs.put(b"b\n").expect("存得进");
    assert_eq!(stored, ContentHash::of(b"b\n"));
    assert_eq!(
        sides(&blobs, &forged),
        Ok((b"a\n".to_vec(), b"b\n".to_vec()))
    );
    // 以前的日志没写原文：照旧取 blob。
    let old = changed(Some(b"a\n"), b"b\n", (None, None));
    assert_eq!(
        sides(&blobs, &old),
        Err(Skipped::Missing),
        "改前的 blob 没有"
    );
    blobs.put(b"a\n").expect("存得进");
    assert_eq!(sides(&blobs, &old), Ok((b"a\n".to_vec(), b"b\n".to_vec())));
    drop(std::fs::remove_dir_all(scratch));
}
