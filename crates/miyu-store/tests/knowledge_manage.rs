//! 人管理知识库（施工 K-7 上，`docs/blueprint/knowledge.md`「怎么走」第八条）：建库带名字说明、有了的不建；改名字说明照
//! 原样留着别的格；删库整个目录进回收站、挂进来的不碰挂的目录；挂目录、挂不上的什么都不建；加文档（PDF 也行）、有了的不换、
//! 说换的换、挂进来的不加；重建索引数对得上、向量没了。

use std::path::PathBuf;

use miyu_store::knowledge::{BaseFields, Knowledge, KnowledgeError};
use miyu_store::root::DataRoot;

use crate::support::*;

/// 只写名字、说明的几格。
fn fields<'a>(name: Option<&'a str>, description: Option<&'a str>) -> BaseFields<'a> {
    BaseFields {
        name,
        description,
        auto: None,
    }
}

fn top(root: &DataRoot) -> PathBuf {
    root.path().join("kb")
}

/// 一块盘上没有能放的回收站的机器（临时目录那块盘）：照实报了算过，交回没挪成。
fn trashed(result: Result<String, KnowledgeError>) -> Option<String> {
    match result {
        Ok(trash) => Some(trash),
        Err(KnowledgeError::Trash(_)) => None,
        Err(other) => panic!("{other}"),
    }
}

fn home(scratch: &Scratch) -> PathBuf {
    let home = scratch.0.join("home");
    std::fs::create_dir_all(&home).expect("建得了");
    home
}

#[test]
fn creating_and_renaming_a_base() {
    let scratch = Scratch::new("kb-found");
    let root = root_in(&scratch);
    let knowledge = Knowledge::new(&root);
    let base = knowledge
        .found("notes", fields(Some("笔记"), Some("学日语的 \"笔记\"")))
        .expect("建得了");
    assert_eq!(
        (
            base.id.as_str(),
            base.name.as_str(),
            base.description.as_str()
        ),
        ("notes", "笔记", "学日语的 \"笔记\"")
    );
    assert!(matches!(
        knowledge.found("notes", fields(None, None)),
        Err(KnowledgeError::BaseExists)
    ));
    assert!(matches!(
        knowledge.found("笔记", fields(None, None)),
        Err(KnowledgeError::UnknownBase)
    ));
    // 改名字：说明照留；文件里的注释、别的格照原样。
    let file = top(&root).join("notes/kb.toml");
    let text = std::fs::read_to_string(&file).expect("读得出");
    std::fs::write(&file, format!("# 我的注释\n{text}")).expect("写得进");
    let renamed = knowledge
        .rename("notes", fields(Some("日语"), None))
        .expect("改得了");
    assert_eq!(
        (renamed.name.as_str(), renamed.description.as_str()),
        ("日语", "学日语的 \"笔记\"")
    );
    assert!(
        std::fs::read_to_string(&file)
            .expect("读得出")
            .starts_with("# 我的注释\n")
    );
    // 空字符串是拿掉。
    let cleared = knowledge
        .rename("notes", fields(None, Some("")))
        .expect("改得了");
    assert_eq!(
        (cleared.name.as_str(), cleared.description.as_str()),
        ("日语", "")
    );
    assert!(
        !std::fs::read_to_string(&file)
            .expect("读得出")
            .contains("description"),
        "拿掉的那一格不在文件里"
    );
    assert!(matches!(
        knowledge.rename("nope", fields(Some("x"), None)),
        Err(KnowledgeError::UnknownBase)
    ));
}

#[test]
fn mounting_a_folder_and_a_folder_that_is_not_there() {
    let scratch = Scratch::new("kb-mount-it");
    let root = root_in(&scratch);
    let notes = scratch.0.join("notes");
    std::fs::create_dir_all(&notes).expect("建得了");
    std::fs::write(notes.join("a.md"), "团子是橘猫。\n").expect("写得进");
    let knowledge = Knowledge::new(&root);
    let folder = notes.display().to_string();
    let (base, _) = knowledge
        .mount("notes", &folder, fields(Some("笔记"), None))
        .expect("挂得上");
    assert_eq!(
        (base.name.as_str(), base.documents, &base.error),
        ("笔记", 1, &None)
    );
    assert!(matches!(
        knowledge.mount("notes", &folder, fields(None, None)),
        Err(KnowledgeError::BaseExists)
    ));
    let gone = scratch.0.join("gone").display().to_string();
    assert!(matches!(
        knowledge.mount("other", &gone, fields(None, None)),
        Err(KnowledgeError::FolderUnavailable(_))
    ));
    assert!(matches!(
        knowledge.mount("other", "relative", fields(None, None)),
        Err(KnowledgeError::FolderUnavailable(_))
    ));
    assert!(!top(&root).join("other").exists(), "挂不上的什么都不建");
    // 改名字时 `mount` 照留。
    let renamed = knowledge
        .rename("notes", fields(None, Some("说明")))
        .expect("改得了");
    assert_eq!(
        (renamed.documents, renamed.description.as_str()),
        (1, "说明")
    );
}

#[test]
fn adding_documents_of_any_kind_and_replacing_only_when_asked() {
    let scratch = Scratch::new("kb-add");
    let root = root_in(&scratch);
    let knowledge = Knowledge::new(&root);
    knowledge
        .found("notes", fields(None, None))
        .expect("建得了");
    knowledge
        .add("notes", "docs/a.md", "团子\n".as_bytes(), false)
        .expect("加得了");
    let pdf = b"%PDF-1.4 not really";
    knowledge
        .add("notes", "cat.pdf", pdf, false)
        .expect("PDF 也加得了");
    assert_eq!(
        std::fs::read(top(&root).join("notes/cat.pdf")).expect("在"),
        pdf
    );
    assert!(matches!(
        knowledge.add("notes", "docs/a.md", b"x", false),
        Err(KnowledgeError::Exists)
    ));
    knowledge
        .add("notes", "docs/a.md", "小橘\n".as_bytes(), true)
        .expect("说换的换");
    assert_eq!(
        std::fs::read_to_string(top(&root).join("notes/docs/a.md")).expect("在"),
        "小橘\n"
    );
    for path in ["../x.md", "a.exe", ".hidden/a.md"] {
        assert!(
            matches!(
                knowledge.add("notes", path, b"x", false),
                Err(KnowledgeError::UnknownDocument)
            ),
            "{path}"
        );
    }
    let notes = scratch.0.join("mounted");
    std::fs::create_dir_all(&notes).expect("建得了");
    knowledge
        .mount("mounted", &notes.display().to_string(), fields(None, None))
        .expect("挂得上");
    assert!(matches!(
        knowledge.add("mounted", "a.md", b"x", false),
        Err(KnowledgeError::ReadOnly)
    ));
    assert!(
        std::fs::read_dir(&notes).expect("在").next().is_none(),
        "挂的目录没动"
    );
}

#[test]
fn deleting_a_base_moves_it_to_the_trash_and_leaves_a_mounted_folder_alone() {
    let scratch = Scratch::new("kb-delete");
    let root = root_in(&scratch);
    let home = home(&scratch);
    let knowledge = Knowledge::new(&root);
    knowledge
        .found("notes", fields(None, None))
        .expect("建得了");
    knowledge
        .add("notes", "a.md", "团子\n".as_bytes(), false)
        .expect("加得了");
    knowledge.list().expect("补得了");
    let index = root.index().join("kb/notes.db");
    assert!(index.exists());
    if trashed(knowledge.delete("notes", Some(&home))).is_some() {
        assert!(!top(&root).join("notes").exists(), "整个目录挪走了");
        assert!(!index.exists(), "索引删掉");
        assert!(knowledge.list().expect("列得出").0.is_empty());
        // 同一个编号再建：索引是新开的一份，照样补、照样搜得到。
        knowledge
            .found("notes", fields(None, None))
            .expect("建得了");
        knowledge
            .add("notes", "b.md", "阿黄\n".as_bytes(), false)
            .expect("加得了");
        let found = knowledge.search("阿黄", None, 5, None).expect("搜得了").0;
        assert_eq!(found.len(), 1, "新建的照样搜得到");
    }
    let notes = scratch.0.join("mine");
    std::fs::create_dir_all(&notes).expect("建得了");
    std::fs::write(notes.join("b.md"), "阿黄\n").expect("写得进");
    knowledge
        .mount("mine", &notes.display().to_string(), fields(None, None))
        .expect("挂得上");
    if trashed(knowledge.delete("mine", Some(&home))).is_some() {
        assert!(!top(&root).join("mine").exists(), "登记拿掉");
        assert_eq!(
            std::fs::read_to_string(notes.join("b.md")).expect("在"),
            "阿黄\n",
            "挂的目录没动"
        );
    }
    assert!(matches!(
        knowledge.delete("nope", Some(&home)),
        Err(KnowledgeError::UnknownBase)
    ));
}

#[test]
fn reindexing_rebuilds_from_the_files_and_drops_the_vectors() {
    let scratch = Scratch::new("kb-reindex");
    let root = root_in(&scratch);
    let knowledge = Knowledge::new(&root);
    knowledge
        .found("notes", fields(None, None))
        .expect("建得了");
    knowledge
        .add("notes", "a.md", "团子\n".as_bytes(), false)
        .expect("加得了");
    knowledge.list().expect("补得了");
    let index = knowledge.index_of("notes");
    index
        .put_vector("a.md#0", "m", &[1.0, 0.0])
        .expect("写得进");
    let (base, _) = knowledge.reindex("notes").expect("重建得了");
    assert_eq!((base.documents, base.chunks), (1, 1));
    let index = knowledge.index_of("notes");
    assert_eq!(
        index.missing("m", 0, 10).expect("读得了").len(),
        1,
        "向量没了，下一次搜完再补"
    );
    assert!(matches!(
        knowledge.reindex("nope"),
        Err(KnowledgeError::UnknownBase)
    ));
}

#[test]
fn auto_recall_is_switched_in_kb_toml() {
    // 施工 K-6 下：`auto` 写进 `kb.toml`、列出来带上；`false` 拿掉那一格（默认关），名字照留。
    let scratch = Scratch::new("kb-auto");
    let root = root_in(&scratch);
    let knowledge = Knowledge::new(&root);
    let on = BaseFields {
        name: Some("世界观"),
        auto: Some(true),
        ..BaseFields::default()
    };
    let base = knowledge.found("world", on).expect("建得了");
    assert!(base.auto);
    let file = top(&root).join("world/kb.toml");
    assert!(
        std::fs::read_to_string(&file)
            .expect("读得出")
            .contains("auto = true")
    );
    let off = BaseFields {
        auto: Some(false),
        ..BaseFields::default()
    };
    let base = knowledge.rename("world", off).expect("改得了");
    assert!(!base.auto);
    assert_eq!(base.name, "世界观");
    assert!(
        !std::fs::read_to_string(&file)
            .expect("读得出")
            .contains("auto")
    );
    assert!(!knowledge.list().expect("列得出").0[0].auto);
}

#[test]
fn every_section_of_a_base_is_listed_with_its_heading() {
    // 施工 K-6 下：自动想起照标题名挑，一个库的每一段（键、标题路径、字）列得出；先照文件补。
    let scratch = Scratch::new("kb-sections");
    let root = root_in(&scratch);
    let knowledge = Knowledge::new(&root);
    knowledge
        .found("world", BaseFields::default())
        .expect("建得了");
    knowledge
        .add(
            "world",
            "people.md",
            "# 人物\n\n## 艾琳\n\n守灯人。\n\n## 巴德\n\n老船长。\n".as_bytes(),
            false,
        )
        .expect("加得了");
    let (sections, skipped) = knowledge.sections("world").expect("列得出");
    assert!(skipped.is_empty());
    let shown: Vec<(String, String, String)> = sections
        .into_iter()
        .map(|hit| (format!("{}#{}", hit.path, hit.chunk), hit.heading, hit.text))
        .collect();
    assert_eq!(
        shown,
        [
            (
                "people.md#0".to_string(),
                "人物 › 艾琳".to_string(),
                "守灯人。".to_string()
            ),
            (
                "people.md#1".to_string(),
                "人物 › 巴德".to_string(),
                "老船长。".to_string()
            ),
        ]
    );
    assert!(matches!(
        knowledge.sections("nope"),
        Err(KnowledgeError::UnknownBase)
    ));
}
