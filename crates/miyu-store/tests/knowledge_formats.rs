//! PDF 和网页（施工 K-5，`docs/blueprint/knowledge.md`「对外的样子」文档那一段、「怎么走」第二、三、五、七条）：PDF 一页一页抽
//! 字、照页切、搜到的说第几页、照页读，部首的码位换回平常的字；抽不出字的、坏的跳过；网页换成文字照标题切、读的是换出来的字；
//! 只有文字的文档写得了，PDF、网页照样删得了。抽字照小程序 `miyu-extract`（`cargo test --workspace` 会编它）；没交的跳过 PDF、
//! 网页，读的说读不了。

use std::path::{Path, PathBuf};

use miyu_store::knowledge::{Knowledge, KnowledgeError};
use miyu_store::root::DataRoot;

use crate::support::*;

fn kb(root: &DataRoot, id: &str) -> PathBuf {
    let dir = root.path().join("kb").join(id);
    std::fs::create_dir_all(&dir).expect("建得了");
    dir
}

/// 测试数据里的一份（抽字的小程序那里的 `tests/fixtures/`，怎么做的见那里的 README）。
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../miyu-extract/tests/fixtures")
        .join(name)
}

/// 交了抽字的小程序的知识库。
fn knowledge(root: &DataRoot) -> Knowledge {
    Knowledge::new(root).with_extractor(Some(extractor()))
}

fn put(dir: &Path, name: &str, from: &str) {
    std::fs::copy(fixture(from), dir.join(name)).expect("拷得了");
}

/// 搜 `query`：哪个文档、第几页、标题路径、字。
fn found(knowledge: &Knowledge, query: &str) -> Vec<(String, Option<usize>, String, String)> {
    knowledge
        .search(&admin(), query, None, 10, None)
        .expect("搜得了")
        .0
        .into_iter()
        .map(|hit| (hit.path, hit.page, hit.heading, hit.text))
        .collect()
}

#[test]
fn a_pdf_is_searched_and_read_page_by_page() {
    let scratch = Scratch::new("kb-pdf");
    let root = root_in(&scratch);
    put(&kb(&root, "notes"), "cat.pdf", "cat.pdf");
    let knowledge = knowledge(&root);
    let (bases, skipped) = knowledge.list(&admin()).expect("补得了");
    assert!(skipped.is_empty(), "{skipped:?}");
    assert_eq!((bases[0].documents, bases[0].chunks), (1, 3), "一页一块");
    let cat = found(&knowledge, "橘猫");
    assert_eq!(cat.len(), 1);
    assert_eq!((cat[0].0.as_str(), cat[0].1), ("cat.pdf", Some(1)));
    assert_eq!(cat[0].2, "", "PDF 没有标题路径");
    assert_eq!(
        cat[0].3, "团子是一只橘猫，每天吃鸡胸肉。",
        "部首的码位换回平常的字"
    );
    assert_eq!(found(&knowledge, "团子")[0].1, Some(1), "「子」搜得到");
    assert_eq!(found(&knowledge, "October")[0].1, Some(2));
    assert_eq!(
        found(&knowledge, "第三页")[0].1,
        Some(3),
        "部首补充那一段也换"
    );
    let page = knowledge.read("notes", "cat.pdf", 2).expect("读得到");
    assert_eq!(
        (page.text.as_str(), page.page, page.pages),
        ("Vaccines are given every October.", 2, 3)
    );
    assert!(matches!(
        knowledge.read("notes", "cat.pdf", 4),
        Err(KnowledgeError::BadPage)
    ));
}

#[test]
fn a_pdf_without_text_and_a_broken_one_are_skipped() {
    let scratch = Scratch::new("kb-pdf-skip");
    let root = root_in(&scratch);
    let dir = kb(&root, "notes");
    put(&dir, "scan.pdf", "blank.pdf");
    std::fs::write(dir.join("broken.pdf"), "这不是 PDF").expect("写得进");
    // 比文字的上限（4 MiB）大、比 PDF 的上限（64 MiB）小：照 PDF 的上限，去抽了才知道坏。
    std::fs::write(dir.join("big.pdf"), vec![b' '; 5 * 1024 * 1024]).expect("写得进");
    std::fs::write(dir.join("ok.md"), "团子\n").expect("写得进");
    let knowledge = knowledge(&root);
    let (bases, skipped) = knowledge.list(&admin()).expect("一份坏的不碍事");
    assert_eq!(bases[0].documents, 1, "只有 ok.md");
    let mut paths: Vec<&str> = skipped.iter().map(|one| one.path.as_str()).collect();
    paths.sort_unstable();
    assert_eq!(paths, ["big.pdf", "broken.pdf", "scan.pdf"]);
    let big = skipped
        .iter()
        .find(|one| one.path == "big.pdf")
        .expect("跳过了");
    assert!(big.why.starts_with("unreadable PDF"), "{}", big.why);
    assert!(
        matches!(
            knowledge.read("notes", "broken.pdf", 1),
            Err(KnowledgeError::Io(_))
        ),
        "读不了的照实说"
    );
}

#[test]
fn a_web_page_is_searched_by_heading_and_read_as_text() {
    let scratch = Scratch::new("kb-html");
    let root = root_in(&scratch);
    let dir = kb(&root, "notes");
    let html = "<html><head><title>标题</title><style>.x{color:red}</style><script>var hidden = 1;</script></head>\
        <body><h1>猫</h1><p>团子是<b>橘猫</b>，<a href=\"x.html\">链接</a>。</p><h2>疫苗</h2><p>每年十月打针。</p></body></html>";
    std::fs::write(dir.join("cat.html"), html).expect("写得进");
    std::fs::write(dir.join("old.htm"), "<p>旧的网页</p>").expect("写得进");
    let knowledge = knowledge(&root);
    assert_eq!(
        found(&knowledge, "打针"),
        [(
            "cat.html".to_string(),
            None,
            "猫 › 疫苗".to_string(),
            "每年十月打针。".to_string()
        )]
    );
    assert!(found(&knowledge, "hidden").is_empty(), "脚本里的不算");
    assert!(found(&knowledge, "color").is_empty(), "样式里的不算");
    assert_eq!(found(&knowledge, "旧的网页")[0].0, "old.htm");
    let page = knowledge.read("notes", "cat.html", 1).expect("读得到");
    assert_eq!(
        page.text,
        "# 猫\n\n团子是橘猫，[链接]。\n\n## 疫苗\n\n每年十月打针。\n"
    );
}

#[test]
fn only_text_documents_are_written_but_any_can_be_trashed() {
    let scratch = Scratch::new("kb-pdf-write");
    let root = root_in(&scratch);
    let dir = kb(&root, "notes");
    put(&dir, "cat.pdf", "cat.pdf");
    std::fs::write(dir.join("cat.html"), "<p>团子</p>").expect("写得进");
    let knowledge = knowledge(&root);
    for path in ["cat.pdf", "new.pdf", "cat.html", "new.htm"] {
        assert!(
            matches!(
                knowledge.save("notes", path, "x", true),
                Err(KnowledgeError::NotText)
            ),
            "{path}"
        );
    }
    assert!(matches!(
        knowledge.edit("notes", "cat.html", "团子", "小橘"),
        Err(KnowledgeError::NotText)
    ));
    assert!(!dir.join("new.pdf").exists() && !dir.join("new.htm").exists());
    let home = scratch.0.join("home");
    std::fs::create_dir_all(&home).expect("建得了");
    match knowledge.trash("notes", "cat.pdf", Some(&home)) {
        Ok(_) => assert!(!dir.join("cat.pdf").exists(), "挪走了"),
        // 有的机器上临时目录那块盘没有能放的回收站：照实报。
        Err(KnowledgeError::Trash(_)) => {}
        Err(other) => panic!("PDF 照样删得了：{other}"),
    }
}

#[test]
fn without_the_extractor_pdfs_and_web_pages_are_skipped() {
    let scratch = Scratch::new("kb-no-extractor");
    let root = root_in(&scratch);
    let dir = kb(&root, "notes");
    put(&dir, "cat.pdf", "cat.pdf");
    std::fs::write(dir.join("cat.html"), "<p>团子</p>").expect("写得进");
    std::fs::write(dir.join("ok.md"), "团子\n").expect("写得进");
    let knowledge = Knowledge::new(&root);
    let (bases, skipped) = knowledge.list(&admin()).expect("照样补得了");
    assert_eq!(bases[0].documents, 1);
    assert_eq!(skipped.len(), 2);
    assert!(
        skipped.iter().all(|one| one.why.contains("miyu-extract")),
        "{skipped:?}"
    );
    assert!(matches!(
        knowledge.read("notes", "cat.pdf", 1),
        Err(KnowledgeError::Io(_))
    ));
}

#[test]
fn the_extractor_is_found_beside_the_main_program() {
    let scratch = Scratch::new("kb-beside");
    let bin = scratch.0.join("bin");
    std::fs::create_dir_all(&bin).expect("建得了");
    let main = bin.join(format!("miyu{}", std::env::consts::EXE_SUFFIX));
    assert_eq!(
        miyu_store::knowledge::extractor_beside(&main),
        None,
        "旁边没有的没有"
    );
    let program = bin.join(format!("miyu-extract{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&program, "").expect("写得进");
    assert_eq!(
        miyu_store::knowledge::extractor_beside(&main),
        Some(program)
    );
}

#[test]
fn the_listed_extensions_are_the_known_ones() {
    // 施工 K-7 修：命令行照这张表说只收哪几种，得和认的一样。
    for extension in miyu_store::knowledge::EXTENSIONS {
        assert!(Knowledge::known(&format!("a.{extension}")), "{extension}");
        assert!(
            Knowledge::known(&format!("a.{}", extension.to_uppercase())),
            "大写的也认：{extension}"
        );
    }
    for name in ["a.docx", "a", ".md", "a.md.bak"] {
        assert!(!Knowledge::known(name), "{name}");
    }
}
