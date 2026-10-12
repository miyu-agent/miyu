//! 挂进来的知识库（施工 K-4，`docs/blueprint/knowledge.md`「对外的样子」、「怎么走」第三条第 6 款、第七条）：`kb.toml` 写了
//! `mount` 的照那个目录搜、读，库目录里别的文件不算，`~/` 照家目录；挂的目录用不了的列出来带 `error`、搜的时候跳过、索引不动，
//! 回来了照搜；写的三件拒，什么都不动。

use std::path::{Path, PathBuf};

use miyu_store::knowledge::{Knowledge, KnowledgeError};
use miyu_store::root::DataRoot;

use crate::support::*;

/// 管理员的库 `id`，`kb.toml` 写 `toml`。
fn base(root: &DataRoot, id: &str, toml: &str) -> PathBuf {
    let dir = root.account_dir(&admin()).join("kb").join(id);
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::write(dir.join("kb.toml"), toml).expect("写得进");
    dir
}

/// `mount = '<dir>'`：TOML 的字面字符串，Windows 的反斜杠不用转义。
fn mount(dir: &Path) -> String {
    format!("mount = '{}'\n", dir.display())
}

fn write(dir: &Path, name: &str, text: &str) {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().expect("有上一级")).expect("建得了");
    std::fs::write(path, text).expect("写得进");
}

/// 搜 `query`：哪个库、哪个文档。
fn found(knowledge: &Knowledge, query: &str) -> Vec<(String, String)> {
    knowledge
        .search(&admin(), query, None, 10, None)
        .expect("搜得了")
        .0
        .into_iter()
        .map(|hit| (hit.kb, hit.path))
        .collect()
}

/// 外面的一个笔记目录：两篇文档，一篇藏在以点开头的目录里。
fn notes(scratch: &Scratch) -> PathBuf {
    let notes = scratch.0.join("notes");
    write(&notes, "a.md", "# 猫\n\n团子是橘猫。\n");
    write(&notes, "sub/b.txt", "阿黄是小狗。\n");
    write(&notes, ".obsidian/c.md", "团子的设置\n");
    notes
}

#[test]
fn a_mounted_folder_is_searched_and_read_and_the_base_dir_holds_only_kb_toml() {
    let scratch = Scratch::new("kb-mount");
    let root = root_in(&scratch);
    let notes = notes(&scratch);
    let dir = base(
        &root,
        "notes",
        &format!("name = \"笔记\"\n{}", mount(&notes)),
    );
    write(&dir, "own.md", "团子在库目录里。\n");
    let knowledge = Knowledge::new(&root);
    let (bases, skipped) = knowledge.list(&admin()).expect("列得出");
    assert!(skipped.is_empty());
    assert_eq!(bases.len(), 1);
    assert_eq!(
        (bases[0].name.as_str(), bases[0].documents, &bases[0].error),
        ("笔记", 2, &None)
    );
    assert_eq!(
        found(&knowledge, "团子"),
        [("notes".to_string(), "a.md".to_string())],
        "库目录里的、以点开头的目录里的不算"
    );
    assert_eq!(found(&knowledge, "阿黄")[0].1, "sub/b.txt");
    let page = knowledge
        .read(&admin(), "notes", "a.md", 1)
        .expect("读得到");
    assert_eq!(page.text, "# 猫\n\n团子是橘猫。\n");
    assert!(matches!(
        knowledge.read(&admin(), "notes", "own.md", 1),
        Err(KnowledgeError::UnknownDocument)
    ));
    // 在原处改了，下一次搜就照新的。
    std::fs::write(notes.join("a.md"), "团子改名叫小橘。\n").expect("写得进");
    assert_eq!(found(&knowledge, "小橘")[0].1, "a.md");
}

#[test]
fn tilde_follows_the_home_directory() {
    let scratch = Scratch::new("kb-mount-home");
    let root = root_in(&scratch);
    notes(&scratch);
    base(&root, "notes", "mount = \"~/notes\"\n");
    let knowledge = Knowledge::new(&root).with_home(Some(scratch.0.clone()));
    assert_eq!(found(&knowledge, "橘猫")[0].1, "a.md");
    let homeless = Knowledge::new(&root);
    let (bases, _) = homeless.list(&admin()).expect("列得出");
    assert!(bases[0].error.is_some(), "家目录不知道的用不了");
}

#[test]
fn an_unavailable_mount_keeps_its_index_and_is_skipped() {
    let scratch = Scratch::new("kb-mount-gone");
    let root = root_in(&scratch);
    let notes = notes(&scratch);
    base(&root, "notes", &mount(&notes));
    let own = base(&root, "own", "");
    write(&own, "x.md", "团子的饭是鸡胸肉。\n");
    let knowledge = Knowledge::new(&root);
    assert_eq!(found(&knowledge, "团子").len(), 2);
    let away = scratch.0.join("unplugged");
    std::fs::rename(&notes, &away).expect("挪得走");
    let (bases, _) = knowledge.list(&admin()).expect("一个用不了的不碍事");
    let unplugged = &bases[0];
    assert_eq!(unplugged.id, "notes");
    assert!(unplugged.error.is_some(), "说为什么");
    assert_eq!((unplugged.documents, unplugged.chunks), (2, 2), "索引不动");
    assert_eq!(
        found(&knowledge, "团子"),
        [("own".to_string(), "x.md".to_string())],
        "跳过它，别的照搜"
    );
    let only = ["notes".to_string()];
    let alone = knowledge
        .search(&admin(), "团子", Some(&only), 10, None)
        .expect("只搜它也不算出错");
    assert!(alone.0.is_empty());
    assert!(matches!(
        knowledge.read(&admin(), "notes", "a.md", 1),
        Err(KnowledgeError::UnknownDocument)
    ));
    std::fs::rename(&away, &notes).expect("挪得回");
    assert_eq!(found(&knowledge, "橘猫")[0].0, "notes", "回来了照搜");
    // 写的不是绝对路径、也不是 `~/` 开头的：用不了。`.` 照工作目录是读得了的，照样不收。
    base(&root, "notes", "mount = \".\"\n");
    assert!(
        knowledge.list(&admin()).expect("列得出").0[0]
            .error
            .is_some()
    );
}

#[test]
fn writing_to_a_mounted_base_is_refused_and_changes_nothing() {
    let scratch = Scratch::new("kb-mount-write");
    let root = root_in(&scratch);
    let notes = notes(&scratch);
    let dir = base(&root, "notes", &mount(&notes));
    let knowledge = Knowledge::new(&root);
    assert!(matches!(
        knowledge.save(&admin(), "notes", "new.md", "x", false),
        Err(KnowledgeError::ReadOnly)
    ));
    assert!(matches!(
        knowledge.save(&admin(), "notes", "a.md", "x", true),
        Err(KnowledgeError::ReadOnly)
    ));
    assert!(matches!(
        knowledge.edit(&admin(), "notes", "a.md", "团子", "小橘"),
        Err(KnowledgeError::ReadOnly)
    ));
    assert!(matches!(
        knowledge.trash(&admin(), "notes", "a.md", Some(&scratch.0)),
        Err(KnowledgeError::ReadOnly)
    ));
    assert_eq!(
        std::fs::read_to_string(notes.join("a.md")).expect("还在"),
        "# 猫\n\n团子是橘猫。\n"
    );
    assert!(!notes.join("new.md").exists() && !dir.join("new.md").exists());
}

/// 挂的目录里读不了的子目录（macOS 上受保护的目录）：跳过、照跳过的文档交回，别的照补（施工 K-4）。以 root 跑的读得了，不算。
#[cfg(unix)]
#[test]
fn an_unreadable_folder_inside_is_skipped() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new("kb-mount-locked");
    let root = root_in(&scratch);
    let notes = notes(&scratch);
    write(&notes, "locked/d.md", "锁着的团子。\n");
    let locked = notes.join("locked");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).expect("改得了");
    if std::fs::read_dir(&locked).is_ok() {
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).expect("改得回");
        return;
    }
    base(&root, "notes", &mount(&notes));
    let knowledge = Knowledge::new(&root);
    let (bases, skipped) = knowledge.list(&admin()).expect("列得出");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).expect("改得回");
    assert_eq!(bases[0].documents, 2);
    assert_eq!(
        skipped
            .iter()
            .map(|one| (one.kb.as_str(), one.path.as_str()))
            .collect::<Vec<_>>(),
        [("notes", "locked")]
    );
}
