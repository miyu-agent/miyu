//! 她写知识库（施工 K-3，`docs/blueprint/knowledge.md`「怎么走」第七条）：建库带说明、已经有的不动；新建一篇、已经有的不写；
//! 追加接在后面（没有就新建）、补上中间的换行；照原文换一段，找不到、不止一处的不换；挪进回收站；路径跑出去的、不是文档的
//! 不写；写完超过上限的不写。每次交回改前改后的内容（撤销照它改回去）。

use std::path::PathBuf;

use miyu_store::knowledge::{Knowledge, KnowledgeError};

use crate::support::*;

fn base(root: &miyu_store::root::DataRoot, kb: &str) -> PathBuf {
    root.path().join("kb").join(kb)
}

#[test]
fn a_base_is_created_with_what_it_is_for() {
    let scratch = Scratch::new("kb-create");
    let root = root_in(&scratch);
    let knowledge = Knowledge::new(&root);
    knowledge
        .create("recipes", Some("家常菜的做法 \"和\" 用量"))
        .expect("建得了");
    assert_eq!(
        knowledge.title("recipes").expect("有了"),
        (
            "recipes".to_string(),
            "家常菜的做法 \"和\" 用量".to_string()
        ),
        "说明照写进 kb.toml，名字没写照编号"
    );
    knowledge
        .create("recipes", Some("换一句"))
        .expect("已经有的照样成");
    assert_eq!(
        knowledge.title("recipes").expect("有").1,
        "家常菜的做法 \"和\" 用量",
        "已经有的不动"
    );
    knowledge.create("plain", None).expect("建得了");
    assert!(
        !base(&root, "plain").join("kb.toml").exists(),
        "没说明的不写 kb.toml"
    );
    assert!(matches!(
        knowledge.create("Bad Name", None),
        Err(KnowledgeError::UnknownBase)
    ));
}

#[test]
fn saving_appending_and_editing_say_before_and_after() {
    let scratch = Scratch::new("kb-save");
    let root = root_in(&scratch);
    let knowledge = Knowledge::new(&root);
    knowledge.create("notes", None).expect("建得了");

    let saved = knowledge
        .save("notes", "grammar/te.md", "て形", false)
        .expect("写得进");
    let file = base(&root, "notes").join("grammar").join("te.md");
    assert_eq!(saved.path, file, "上级目录没有的建");
    assert_eq!(saved.before, None);
    assert_eq!(saved.after, "て形\n".as_bytes(), "末尾补一个换行");
    assert!(matches!(
        knowledge.save("notes", "grammar/te.md", "又一份", false),
        Err(KnowledgeError::Exists)
    ));

    std::fs::write(&file, "て形").expect("写得进");
    let appended = knowledge
        .save("notes", "grammar/te.md", "食べる → 食べて\n", true)
        .expect("写得进");
    assert_eq!(appended.before.as_deref(), Some("て形".as_bytes()));
    assert_eq!(
        std::fs::read_to_string(&file).expect("读得出"),
        "て形\n食べる → 食べて\n",
        "原来末尾没换行的先补一个"
    );
    let fresh = knowledge
        .save("notes", "new.txt", "一行", true)
        .expect("没有的追加就是新建");
    assert_eq!(fresh.before, None);

    let edited = knowledge
        .edit("notes", "grammar/te.md", "食べて", "たべて")
        .expect("改得了");
    assert_eq!(edited.after, "て形\n食べる → たべて\n".as_bytes());
    assert!(matches!(
        knowledge.edit("notes", "grammar/te.md", "没有这句", "x"),
        Err(KnowledgeError::NoMatch)
    ));
    std::fs::write(&file, "猫 猫\n").expect("写得进");
    assert!(matches!(
        knowledge.edit("notes", "grammar/te.md", "猫", "狗"),
        Err(KnowledgeError::Ambiguous(2))
    ));
    assert!(matches!(
        knowledge.edit("notes", "nope.md", "a", "b"),
        Err(KnowledgeError::UnknownDocument)
    ));
}

#[test]
fn wrong_paths_and_too_much_are_refused() {
    let scratch = Scratch::new("kb-refuse");
    let root = root_in(&scratch);
    let knowledge = Knowledge::new(&root);
    knowledge.create("notes", None).expect("建得了");
    for path in [
        "../out.md",
        "a/../../out.md",
        ".hidden.md",
        "a/.x/b.md",
        "pic.png",
        "",
        "/abs.md",
    ] {
        assert!(
            matches!(
                knowledge.save("notes", path, "字", false),
                Err(KnowledgeError::UnknownDocument)
            ),
            "{path:?}"
        );
    }
    assert!(matches!(
        knowledge.save("nope", "a.md", "字", false),
        Err(KnowledgeError::UnknownBase)
    ));
    let big = "字".repeat(4 * 1024 * 1024 / 3 + 1);
    assert!(matches!(
        knowledge.save("notes", "big.md", &big, false),
        Err(KnowledgeError::TooLarge)
    ));
    assert!(
        !base(&root, "notes").join("big.md").exists(),
        "太大的什么都不写"
    );
}

#[test]
fn a_document_goes_to_the_trash() {
    let scratch = Scratch::new("kb-trash");
    let root = root_in(&scratch);
    let knowledge = Knowledge::new(&root);
    knowledge.create("notes", None).expect("建得了");
    knowledge
        .save("notes", "old.md", "过时了", false)
        .expect("写得进");
    let home = scratch.0.join("home");
    std::fs::create_dir_all(&home).expect("建得了");
    match knowledge.trash("notes", "old.md", Some(&home)) {
        Ok((path, trash)) => {
            assert_eq!(path, base(&root, "notes").join("old.md"));
            assert!(!path.exists(), "挪走了");
            assert!(!trash.is_empty(), "记下回收站里的位置");
        }
        // 有的机器上临时目录那块盘没有能放的回收站：照实报，不删。
        Err(KnowledgeError::Trash(why)) => {
            assert!(base(&root, "notes").join("old.md").exists(), "{why}");
        }
        Err(other) => panic!("{other}"),
    }
    assert!(matches!(
        knowledge.trash("notes", "gone.md", Some(&home)),
        Err(KnowledgeError::UnknownDocument)
    ));
}

/// 数据根的路上有链接（macOS 的临时目录 `/var` 就是 `/private/var` 的链接）：照真实的位置写，写得进（施工 K-3，CI 上 macOS
/// 报的「Too many levels of symbolic links」）。
#[cfg(unix)]
#[test]
fn a_data_root_reached_through_a_link_is_written_where_it_really_is() {
    let scratch = Scratch::new("kb-link");
    let real = scratch.0.join("real");
    std::fs::create_dir_all(&real).expect("建得了");
    let link = scratch.0.join("link");
    std::os::unix::fs::symlink(&real, &link).expect("链得上");
    let root = miyu_store::root::DataRoot::locate(&miyu_store::env::Env {
        platform: miyu_store::env::Platform::current(),
        miyu_home: Some(link.join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    })
    .expect("测试里做得到");
    root.prepare().expect("测试里做得到");
    root.prepare_workspace().expect("测试里做得到");
    let knowledge = Knowledge::new(&root);
    knowledge
        .create("notes", Some("笔记"))
        .expect("建得了，说明写得进");
    let saved = knowledge
        .save("notes", "a.md", "字", false)
        .expect("写得进");
    assert!(
        saved
            .path
            .starts_with(std::fs::canonicalize(&real).expect("在")),
        "{}",
        saved.path.display()
    );
    knowledge.edit("notes", "a.md", "字", "改").expect("改得了");
}
