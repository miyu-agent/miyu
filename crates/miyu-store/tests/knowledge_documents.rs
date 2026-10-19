//! 列出库里的文档（施工 K-10，`docs/blueprint/knowledge.md`「对外的样子」`kb.documents`）：先照文件补，一篇一项照路径排，几种都
//! 认、带大小、块数；这一回读不了的带哪一种、为什么，以前进过索引的留着旧块；读不了的子目录另列；不是文档的、点开头的不列；
//! 挂进来的库带挂的目录；挂的目录用不了的照实报。

use std::path::{Path, PathBuf};

use miyu_store::knowledge::{DocumentKind, Knowledge, KnowledgeError, Reason};
use miyu_store::root::DataRoot;

use crate::support::*;

fn kb(root: &DataRoot, id: &str, toml: &str) -> PathBuf {
    let dir = root.account_dir(&admin()).join("kb").join(id);
    std::fs::create_dir_all(&dir).expect("建得了");
    if !toml.is_empty() {
        std::fs::write(dir.join("kb.toml"), toml).expect("写得进");
    }
    dir
}

fn write(dir: &Path, name: &str, bytes: &[u8]) {
    let file = dir.join(name);
    std::fs::create_dir_all(file.parent().expect("有上一层")).expect("建得了");
    std::fs::write(file, bytes).expect("写得进");
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../miyu-extract/tests/fixtures")
        .join(name)
}

/// 每一篇：路径、哪一种、大小、块数、读不了的哪一种。
fn rows(
    knowledge: &Knowledge,
    id: &str,
) -> Vec<(String, DocumentKind, u64, usize, Option<Reason>)> {
    let (listing, _) = knowledge.listing(&admin(), id).expect("列得出");
    listing
        .documents
        .into_iter()
        .map(|one| {
            (
                one.path,
                one.kind,
                one.size,
                one.chunks,
                one.skipped.map(|(reason, _)| reason),
            )
        })
        .collect()
}

#[test]
fn every_document_is_listed_by_path_with_its_size_and_chunks() {
    let scratch = Scratch::new("kb-docs");
    let root = root_in(&scratch);
    let dir = kb(&root, "notes", "name = \"笔记\"\n");
    write(&dir, "b.md", "# 猫\n团子\n\n# 狗\n豆豆\n".as_bytes());
    write(&dir, "a/c.txt", "第一段\n\n第二段\n".as_bytes());
    write(&dir, "d.htm", "<h1>猫</h1><p>团子</p>".as_bytes());
    std::fs::copy(fixture("cat.pdf"), dir.join("cat.pdf")).expect("拷得了");
    write(&dir, "photo.png", b"not a document");
    write(&dir, ".hidden.md", b"hidden");
    let knowledge = Knowledge::new(&root).with_extractor(Some(extractor()));
    let (listing, skipped) = knowledge.listing(&admin(), "notes").expect("列得出");
    assert!(skipped.is_empty(), "{skipped:?}");
    assert!(listing.folders.is_empty());
    let size = |name: &str| std::fs::metadata(dir.join(name)).expect("在").len();
    assert_eq!(
        rows(&knowledge, "notes"),
        [
            (
                "a/c.txt".into(),
                DocumentKind::Plain,
                size("a/c.txt"),
                1,
                None
            ),
            ("b.md".into(), DocumentKind::Markdown, size("b.md"), 2, None),
            (
                "cat.pdf".into(),
                DocumentKind::Pdf,
                size("cat.pdf"),
                3,
                None
            ),
            ("d.htm".into(), DocumentKind::Html, size("d.htm"), 1, None),
        ],
        "照路径排；不是文档的、点开头的、kb.toml 不列"
    );
    let at = listing.documents[1].at;
    let modified = std::fs::metadata(dir.join("b.md"))
        .and_then(|meta| meta.modified())
        .expect("读得出");
    let millis = modified
        .duration_since(std::time::UNIX_EPOCH)
        .expect("1970 年以后")
        .as_millis();
    assert_eq!(
        at,
        miyu_kernel::time::Timestamp::from_unix_millis(i64::try_from(millis).expect("放得下"))
            .expect("合写法"),
        "改的时刻照文件"
    );
    assert_eq!(
        (DocumentKind::Plain.code(), DocumentKind::Html.code()),
        ("text", "html")
    );
}

#[test]
fn what_cannot_be_read_says_why_and_keeps_its_old_chunks() {
    let scratch = Scratch::new("kb-docs-skip");
    let root = root_in(&scratch);
    let dir = kb(&root, "notes", "");
    write(&dir, "kept.md", "# 猫\n团子\n\n# 狗\n豆豆\n".as_bytes());
    let knowledge = Knowledge::new(&root).with_extractor(Some(extractor()));
    assert_eq!(rows(&knowledge, "notes")[0].3, 2, "先进了索引");
    // 改坏了：读不了，旧的两块留着。
    write(&dir, "kept.md", &[0xff, 0xfe, 0x00]);
    write(&dir, "big.txt", &vec![b'a'; 4 * 1024 * 1024 + 1]);
    std::fs::copy(fixture("blank.pdf"), dir.join("scan.pdf")).expect("拷得了");
    write(&dir, "broken.pdf", "这不是 PDF".as_bytes());
    let (listing, skipped) = knowledge.listing(&admin(), "notes").expect("列得出");
    let found: Vec<(&str, usize, Option<Reason>)> = listing
        .documents
        .iter()
        .map(|one| {
            (
                one.path.as_str(),
                one.chunks,
                one.skipped.as_ref().map(|(reason, _)| *reason),
            )
        })
        .collect();
    assert_eq!(
        found,
        [
            ("big.txt", 0, Some(Reason::TooLarge)),
            ("broken.pdf", 0, Some(Reason::ExtractFailed)),
            ("kept.md", 2, Some(Reason::NotUtf8)),
            ("scan.pdf", 0, Some(Reason::NoText)),
        ]
    );
    assert!(
        listing
            .documents
            .iter()
            .all(|one| one.skipped.as_ref().is_some_and(|(_, why)| !why.is_empty())),
        "都带为什么"
    );
    assert_eq!(skipped.len(), 4, "跳过的照旧交回去记日志");
    assert!(listing.folders.is_empty(), "读不了的文档不算子目录");
    assert_eq!(
        (Reason::TooLarge.code(), Reason::ExtractFailed.code()),
        ("too_large", "extract_failed")
    );
}

#[test]
fn without_the_extractor_a_pdf_says_so() {
    let scratch = Scratch::new("kb-docs-no-extractor");
    let root = root_in(&scratch);
    let dir = kb(&root, "notes", "");
    std::fs::copy(fixture("cat.pdf"), dir.join("cat.pdf")).expect("拷得了");
    let knowledge = Knowledge::new(&root);
    assert_eq!(
        rows(&knowledge, "notes"),
        [(
            "cat.pdf".into(),
            DocumentKind::Pdf,
            std::fs::metadata(dir.join("cat.pdf")).expect("在").len(),
            0,
            Some(Reason::NoExtractor)
        )]
    );
}

#[test]
fn an_empty_base_has_nothing_and_an_unknown_one_is_refused() {
    let scratch = Scratch::new("kb-docs-empty");
    let root = root_in(&scratch);
    kb(&root, "empty", "");
    let knowledge = Knowledge::new(&root);
    let (listing, _) = knowledge.listing(&admin(), "empty").expect("列得出");
    assert!(listing.documents.is_empty() && listing.folders.is_empty());
    assert!(matches!(
        knowledge.listing(&admin(), "nope"),
        Err(KnowledgeError::UnknownBase)
    ));
}

#[test]
fn a_mounted_base_says_its_folder_and_an_unplugged_one_is_unavailable() {
    let scratch = Scratch::new("kb-docs-mount");
    let root = root_in(&scratch);
    let notes = scratch.0.join("notes");
    write(&notes, "a.md", "团子\n".as_bytes());
    let written = notes.display().to_string();
    kb(&root, "notes", &format!("mount = '{written}'\n"));
    kb(&root, "own", "");
    let knowledge = Knowledge::new(&root);
    let (bases, _) = knowledge.list(&admin()).expect("列得出");
    assert_eq!(
        bases[0].mount.as_deref(),
        Some(written.as_str()),
        "照 kb.toml 写的原样"
    );
    assert_eq!(bases[1].mount, None, "自己的没有");
    assert_eq!(rows(&knowledge, "notes")[0].0, "a.md", "照挂的目录列");
    let away = scratch.0.join("unplugged");
    std::fs::rename(&notes, &away).expect("挪得走");
    assert!(matches!(
        knowledge.listing(&admin(), "notes"),
        Err(KnowledgeError::Unavailable(_))
    ));
}

/// 读不了的子目录另列，不当文档（以 root 跑的读得了，不算）。
#[cfg(unix)]
#[test]
fn an_unreadable_folder_is_listed_apart() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new("kb-docs-locked");
    let root = root_in(&scratch);
    let dir = kb(&root, "notes", "");
    write(&dir, "a.md", "团子\n".as_bytes());
    write(&dir, "locked/d.md", "锁着的。\n".as_bytes());
    let locked = dir.join("locked");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).expect("改得了");
    if std::fs::read_dir(&locked).is_ok() {
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).expect("改得回");
        return;
    }
    let listed = Knowledge::new(&root).listing(&admin(), "notes");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).expect("改得回");
    let (listing, _) = listed.expect("列得出");
    assert_eq!(
        listing
            .documents
            .iter()
            .map(|one| one.path.as_str())
            .collect::<Vec<_>>(),
        ["a.md"]
    );
    assert_eq!(
        listing
            .folders
            .iter()
            .map(|one| (one.path.as_str(), one.reason, one.folder))
            .collect::<Vec<_>>(),
        [("locked", Reason::Unreadable, true)]
    );
}
