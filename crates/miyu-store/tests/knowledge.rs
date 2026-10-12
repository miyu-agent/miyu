//! 知识库（施工 K-1 上，`docs/blueprint/knowledge.md`「怎么走」）：列出合写法的库、`kb.toml` 读得出读不出；照文件补：新的放
//! 进去、变了的重切、没了的拿掉、没变的不重读；太大的、不是 UTF-8 的跳过；搜得到、几个库轮流合、只搜一个；读一页、分页、
//! 截在字的边界上；路径跑出去的、不是文档的读不到。

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use miyu_store::knowledge::{Knowledge, KnowledgeError, Meaning};
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
    let only = only.map(|kb| vec![kb.to_string()]);
    knowledge
        .search(&admin(), query, only.as_deref(), 10, None)
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
        .search(&admin(), "团子", None, 4, None)
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
    // 照人格收窄的几个（施工 K-1 下）：同名次照给的先后，有一个不在的不搜。
    let narrowed = ["b".to_string(), "a".to_string()];
    let order: Vec<String> = knowledge
        .search(&admin(), "团子", Some(&narrowed), 4, None)
        .expect("搜得了")
        .0
        .into_iter()
        .map(|hit| hit.kb)
        .collect();
    assert_eq!(order, ["b", "a", "b", "a"]);
    for only in [
        vec!["nope".to_string()],
        vec!["a".to_string(), "nope".to_string()],
    ] {
        assert!(matches!(
            knowledge.search(&admin(), "团子", Some(&only), 4, None),
            Err(KnowledgeError::UnknownBase)
        ));
    }
    assert_eq!(knowledge.ids(&admin()).expect("列得出"), ["a", "b"]);
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

#[test]
fn with_a_query_vector_a_chunk_is_found_by_meaning() {
    // 施工 K-2：关键词对不上、向量像的照意思找得到；没有问句向量的只走关键词；只有向量那一路、不够像的不要（下限 0.40）。
    let scratch = Scratch::new("kb-meaning");
    let root = root_in(&scratch);
    let notes = kb(&root, "notes");
    write(&notes, "cat.md", "团子的饭是鸡胸肉\n");
    let knowledge = Knowledge::new(&root);
    knowledge.list(&admin()).expect("补得了");
    knowledge
        .index_of(&admin(), "notes")
        .put_vector("cat.md#0", "m", &[1.0, 0.0])
        .expect("写得进");
    let near = |vector: &'static [f32]| Meaning { model: "m", vector };
    let search = |near: Option<Meaning<'_>>| {
        knowledge
            .search(&admin(), "猫吃什么", None, 5, near)
            .expect("搜得了")
            .0
            .into_iter()
            .map(|hit| (hit.kb, hit.path, hit.text))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        search(Some(near(&[1.0, 0.0]))),
        [(
            "notes".to_string(),
            "cat.md".to_string(),
            "团子的饭是鸡胸肉".to_string()
        )]
    );
    assert!(search(None).is_empty(), "没有问句向量的只走关键词");
    assert!(search(Some(near(&[0.0, 1.0]))).is_empty(), "不够像的不要");
}

#[test]
fn the_meaning_side_lifts_a_lower_keyword_hit() {
    // 施工 K-2：每个库两路各找 `limit` 的四倍再合，关键词排第二、照意思排第一的那块合起来排得过只有关键词排第一的。只各找
    // `limit` 块的话关键词那一路只交第一名，它就上不来。
    let scratch = Scratch::new("kb-wide");
    let root = root_in(&scratch);
    let notes = kb(&root, "notes");
    write(&notes, "a.md", "团子团子团子\n");
    write(&notes, "b.md", "团子是楼上的一只猫，每天在窗台上睡觉\n");
    let knowledge = Knowledge::new(&root);
    knowledge.list(&admin()).expect("补得了");
    // a 没补上向量：照意思那一路只有 b。
    knowledge
        .index_of(&admin(), "notes")
        .put_vector("b.md#0", "m", &[1.0, 0.0])
        .expect("写得进");
    let first = |near: Option<Meaning<'_>>| {
        knowledge
            .search(&admin(), "团子", None, 1, near)
            .expect("搜得了")
            .0
            .into_iter()
            .map(|hit| hit.path)
            .collect::<Vec<_>>()
    };
    assert_eq!(first(None), ["a.md"], "只照关键词 a 在前");
    let near = Meaning {
        model: "m",
        vector: &[1.0, 0.0],
    };
    assert_eq!(first(Some(near)), ["b.md"]);
}

/// 量尺（施工 K-4）：3000 篇、每篇 8 节的库头一次补、不变的重扫、改了一篇、搜一次各多久。断言结果，耗时打出来看。K-4 前
/// 头一次补 5 秒上下（整份换掉照 `substr` 比，每篇扫一遍整张表），照键的范围删以后 0.6 秒上下（Linux、release）。
#[test]
#[ignore]
fn measure_three_thousand_documents() {
    let scratch = Scratch::new("kb-measure");
    let root = root_in(&scratch);
    let dir = kb(&root, "vault");
    let paragraph =
        "团子是一只橘猫，每天在窗台上晒太阳。The quick brown fox jumps over the lazy dog.\n\n";
    for n in 0..3000 {
        let mut text = format!("# 笔记 {n}\n\n");
        for section in 0..8 {
            text.push_str(&format!("## 小节 {section}\n\n{paragraph}{paragraph}"));
        }
        write(&dir, &format!("d{}/n{n}.md", n % 20), &text);
    }
    let knowledge = Knowledge::new(&root);
    let started = std::time::Instant::now();
    let (bases, _) = knowledge.list(&admin()).expect("补得了");
    let first = started.elapsed();
    assert_eq!((bases[0].documents, bases[0].chunks), (3000, 24000));
    let started = std::time::Instant::now();
    knowledge.list(&admin()).expect("补得了");
    let again = started.elapsed();
    write(&dir, "d0/n0.md", "# 改了\n\n新的内容\n");
    let started = std::time::Instant::now();
    let (bases, _) = knowledge.list(&admin()).expect("补得了");
    let changed = started.elapsed();
    assert_eq!(bases[0].chunks, 23993);
    let started = std::time::Instant::now();
    assert_eq!(hits(&knowledge, "橘猫", None).len(), 10);
    let search = started.elapsed();
    eprintln!("头一次 {first:?}，不变的重扫 {again:?}，改了一篇 {changed:?}，搜一次 {search:?}");
}
