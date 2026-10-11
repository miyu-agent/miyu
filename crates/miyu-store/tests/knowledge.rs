//! 知识库（施工 K-1 上，`docs/blueprint/knowledge.md`「怎么走」）：列出合写法的库、`kb.toml` 读得出读不出；照文件补：新的放
//! 进去、变了的重切、没了的拿掉、没变的不重读；太大的、不是 UTF-8 的跳过；搜得到、几个库轮流合、只搜一个；读一页、分页、
//! 截在字的边界上；路径跑出去的、不是文档的读不到。

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use miyu_store::knowledge::{Knowledge, KnowledgeError};
use miyu_store::root::DataRoot;

use crate::support::*;

fn kb(root: &DataRoot, id: &str) -> PathBuf {
    let dir = root.account_dir(&admin()).join("kb").join(id);
    std::fs::create_dir_all(&dir).expect("建得了");
    dir
}

fn write(dir: &Path, name: &str, text: &str) {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().expect("有上一级")).expect("建得了");
    std::fs::write(path, text).expect("写得进");
}

fn hits(knowledge: &Knowledge, query: &str, only: Option<&str>) -> Vec<(String, String, usize)> {
    knowledge
        .search(&admin(), query, only, 10)
        .expect("搜得了")
        .0
        .into_iter()
        .map(|hit| (hit.kb, hit.path, hit.chunk))
        .collect()
}

#[test]
fn only_well_formed_bases_are_listed_with_what_kb_toml_says() {
    let scratch = Scratch::new("kb-list");
    let root = root_in(&scratch);
    let notes = kb(&root, "notes");
    write(
        &notes,
        "kb.toml",
        "name = \"日语笔记\"\ndescription = \"学日语的笔记\"\n",
    );
    write(&notes, "grammar/te.md", "# て形\n\n食べる → 食べて\n");
    write(&notes, ".hidden.md", "藏起来的");
    write(&notes, "picture.png", "不是文档");
    kb(&root, "Bad Name");
    write(&kb(&root, "plain"), "words.txt", "猫 ねこ\n\n犬 いぬ\n");
    write(&kb(&root, "broken"), "kb.toml", "title = \"错的格\"\n");
    let knowledge = Knowledge::new(&root);
    let (bases, skipped) = knowledge.list(&admin()).expect("列得出");
    assert!(skipped.is_empty(), "{skipped:?}");
    let ids: Vec<&str> = bases.iter().map(|base| base.id.as_str()).collect();
    assert_eq!(
        ids,
        ["broken", "notes", "plain"],
        "不合写法的目录不算，照编号排"
    );
    let notes = &bases[1];
    assert_eq!(
        (notes.name.as_str(), notes.description.as_str()),
        ("日语笔记", "学日语的笔记")
    );
    assert_eq!(
        (notes.documents, notes.chunks),
        (1, 1),
        "以点开头的、别的扩展名不算"
    );
    assert_eq!(bases[2].name, "plain", "没有 kb.toml 的照编号");
    assert!(bases[0].error.is_some(), "kb.toml 写错的带为什么");
    assert!(bases[1].error.is_none());
    assert!(
        Knowledge::new(&root_in(&Scratch::new("kb-none")))
            .list(&admin())
            .expect("列得出")
            .0
            .is_empty(),
        "目录不在的当一个都没有"
    );
}

#[test]
fn the_index_follows_the_files() {
    let scratch = Scratch::new("kb-follow");
    let root = root_in(&scratch);
    let notes = kb(&root, "notes");
    write(&notes, "cat.md", "# 猫\n\n团子是一只橘猫\n");
    let knowledge = Knowledge::new(&root);
    assert_eq!(
        hits(&knowledge, "橘猫", None),
        [("notes".into(), "cat.md".into(), 0)]
    );

    write(&notes, "cat.md", "# 猫\n\n团子现在是一只胖猫了\n");
    assert!(hits(&knowledge, "橘猫", None).is_empty(), "变了的重切");
    assert_eq!(hits(&knowledge, "胖猫", None).len(), 1);

    // 字换了、大小和改的时刻照旧：指纹没变，不重读。
    let path = notes.join("cat.md");
    let before = std::fs::metadata(&path)
        .expect("在")
        .modified()
        .expect("有时刻");
    write(&notes, "cat.md", "# 猫\n\n团子现在是一只瘦猫了\n");
    std::fs::File::options()
        .write(true)
        .open(&path)
        .expect("开得了")
        .set_modified(before)
        .expect("改得了");
    assert_eq!(hits(&knowledge, "胖猫", None).len(), 1, "指纹没变不重读");

    std::fs::remove_file(&path).expect("删得掉");
    assert!(hits(&knowledge, "胖猫", None).is_empty(), "没了的拿掉");
}

#[test]
fn too_big_or_not_utf8_documents_are_skipped_and_others_go_in() {
    let scratch = Scratch::new("kb-skip");
    let root = root_in(&scratch);
    let notes = kb(&root, "notes");
    write(&notes, "ok.md", "能进的一篇");
    std::fs::write(notes.join("big.txt"), "大".repeat(2 * 1024 * 1024)).expect("写得进");
    std::fs::write(notes.join("bytes.txt"), [0xff, 0xfe, 0x00, 0x41]).expect("写得进");
    let knowledge = Knowledge::new(&root);
    let (bases, skipped) = knowledge.list(&admin()).expect("列得出");
    let mut paths: Vec<&str> = skipped.iter().map(|one| one.path.as_str()).collect();
    paths.sort_unstable();
    assert_eq!(paths, ["big.txt", "bytes.txt"], "{skipped:?}");
    assert!(
        skipped
            .iter()
            .all(|one| one.kb == "notes" && !one.why.is_empty())
    );
    assert_eq!(bases[0].documents, 1);
    assert_eq!(hits(&knowledge, "能进", None).len(), 1);
}

#[test]
fn several_bases_take_turns_and_one_can_be_searched_alone() {
    let scratch = Scratch::new("kb-turns");
    let root = root_in(&scratch);
    let a = kb(&root, "a");
    let b = kb(&root, "b");
    for n in 0..3 {
        write(&a, &format!("{n}.txt"), &format!("团子的事 第{n}篇"));
        write(
            &b,
            &format!("{n}.txt"),
            &format!("团子的事 另一个库 第{n}篇"),
        );
    }
    let knowledge = Knowledge::new(&root);
    let found = knowledge
        .search(&admin(), "团子", None, 4)
        .expect("搜得了")
        .0;
    let order: Vec<&str> = found.iter().map(|hit| hit.kb.as_str()).collect();
    assert_eq!(
        order,
        ["a", "b", "a", "b"],
        "每个库的第一名在前，同名次照编号"
    );
    assert!(
        hits(&knowledge, "团子", Some("b"))
            .iter()
            .all(|(kb, _, _)| kb == "b"),
        "只搜一个"
    );
    assert!(matches!(
        knowledge.search(&admin(), "团子", Some("nope"), 4),
        Err(KnowledgeError::UnknownBase)
    ));
    let hit = &found[0];
    assert_eq!(hit.heading, "");
    assert!(hit.text.contains("团子的事"));
}

#[test]
fn a_document_is_read_page_by_page() {
    let scratch = Scratch::new("kb-read");
    let root = root_in(&scratch);
    let notes = kb(&root, "notes");
    let long = "字".repeat(20_000);
    write(&notes, "long.md", &long);
    write(&notes, "short.txt", "");
    std::fs::write(
        root.account_dir(&admin()).join("kb").join("secret.md"),
        "库外面的",
    )
    .expect("写得进");
    let knowledge = Knowledge::new(&root);
    let first = knowledge
        .read(&admin(), "notes", "long.md", 1)
        .expect("读得出");
    assert_eq!(first.pages, 2);
    assert!(first.text.len() <= 32 * 1024);
    let second = knowledge
        .read(&admin(), "notes", "long.md", 2)
        .expect("读得出");
    assert_eq!(
        format!("{}{}", first.text, second.text),
        long,
        "截在字的边界上，一个字都不丢"
    );
    let empty = knowledge
        .read(&admin(), "notes", "short.txt", 1)
        .expect("读得出");
    assert_eq!((empty.text.as_str(), empty.pages), ("", 1));
    for bad in [
        "../secret.md",
        "/etc/passwd",
        "missing.md",
        "kb.toml",
        "./long.md/../x.md",
    ] {
        assert!(
            matches!(
                knowledge.read(&admin(), "notes", bad, 1),
                Err(KnowledgeError::UnknownDocument)
            ),
            "{bad}"
        );
    }
    assert!(matches!(
        knowledge.read(&admin(), "notes", "long.md", 3),
        Err(KnowledgeError::BadPage)
    ));
    assert!(matches!(
        knowledge.read(&admin(), "notes", "long.md", 0),
        Err(KnowledgeError::BadPage)
    ));
    assert!(matches!(
        knowledge.read(&admin(), "nope", "long.md", 1),
        Err(KnowledgeError::UnknownBase)
    ));
    let _ = SystemTime::now() + Duration::from_secs(1);
}
