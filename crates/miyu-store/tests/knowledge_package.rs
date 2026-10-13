//! 软件包带来的知识库（施工 K-8，`docs/blueprint/knowledge.md`「对外的样子」包带来的那一节、「怎么走」）：装了的包的
//! `kb/<编号>/` 列得出、搜得到、读得到，带上是哪个包的；只读，自动想起的开关人改了记在账号自己的 `kb.toml`；自己的库盖住同编号的
//! 包里的，两个包同编号的先到先得，要建的编号和包里的撞了说已经有了；索引在包那一格，卸包时连开关一起清掉。

use std::path::{Path, PathBuf};

use miyu_store::knowledge::{BaseFields, Knowledge, KnowledgeError};
use miyu_store::root::DataRoot;

use crate::support::*;

/// 数据根旁边一个包的目录 `<pkg>/`，带一个库 `kb/<id>/`：`kb.toml` 写 `toml`，一篇 `people.md`。交回包的目录。
fn package(scratch: &Scratch, pkg: &str, id: &str, toml: &str) -> PathBuf {
    let dir = scratch.0.join("packages").join(pkg);
    let base = dir.join("kb").join(id);
    std::fs::create_dir_all(&base).expect("建得了");
    std::fs::write(base.join("kb.toml"), toml).expect("写得进");
    std::fs::write(base.join("people.md"), "# 人物\n\n## 艾琳\n\n守灯人。\n").expect("写得进");
    dir
}

fn own(root: &DataRoot, id: &str) -> PathBuf {
    let dir = root.account_dir(&admin()).join("kb").join(id);
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::write(dir.join("notes.md"), "自己的笔记。\n").expect("写得进");
    dir
}

fn listed(knowledge: &Knowledge) -> Vec<(String, Option<String>, bool)> {
    knowledge
        .list(&admin())
        .expect("列得出")
        .0
        .into_iter()
        .map(|base| (base.id, base.package, base.auto))
        .collect()
}

fn fields(auto: Option<bool>, name: Option<&str>) -> BaseFields<'_> {
    BaseFields {
        name,
        description: None,
        auto,
    }
}

#[test]
fn a_package_base_is_listed_searched_and_read() {
    let scratch = Scratch::new("kb-package");
    let root = root_in(&scratch);
    let pack = package(
        &scratch,
        "world-pack",
        "world",
        "name = \"雾港\"\nauto = true\n",
    );
    let knowledge = Knowledge::new(&root).with_packages(vec![("world-pack".into(), pack)]);
    let (bases, _) = knowledge.list(&admin()).expect("列得出");
    assert_eq!(bases.len(), 1);
    assert_eq!(
        (bases[0].id.as_str(), bases[0].name.as_str()),
        ("world", "雾港")
    );
    assert_eq!(bases[0].package.as_deref(), Some("world-pack"));
    assert!(bases[0].auto, "照包里写的");
    assert_eq!(bases[0].documents, 1);
    let (hits, _) = knowledge
        .search(&admin(), "守灯人", None, 5, None)
        .expect("搜得了");
    assert_eq!(hits[0].kb, "world");
    let page = knowledge
        .read(&admin(), "world", "people.md", 1)
        .expect("读得了");
    assert!(page.text.contains("守灯人"));
    assert!(
        root.index(&admin())
            .join("kb/packages/world-pack/world.db")
            .is_file(),
        "索引在包那一格"
    );
    assert!(knowledge.auto(&admin(), "world").expect("在"));
}

#[test]
fn a_package_base_is_read_only_but_its_auto_switch_is_yours() {
    let scratch = Scratch::new("kb-package-ro");
    let root = root_in(&scratch);
    let pack = package(&scratch, "world-pack", "world", "auto = true\n");
    let knowledge = Knowledge::new(&root).with_packages(vec![("world-pack".into(), pack.clone())]);
    knowledge.list(&admin()).expect("列得出");
    let read_only = |result: Result<(), KnowledgeError>| {
        assert!(
            matches!(result, Err(KnowledgeError::Packaged)),
            "{result:?}"
        );
    };
    read_only(knowledge.add(&admin(), "world", "a.md", b"x\n", false));
    read_only(
        knowledge
            .save(&admin(), "world", "a.md", "x", false)
            .map(|_| ()),
    );
    read_only(
        knowledge
            .trash(&admin(), "world", "people.md", None)
            .map(|_| ()),
    );
    read_only(knowledge.delete(&admin(), "world", None).map(|_| ()));
    read_only(
        knowledge
            .rename(&admin(), "world", fields(None, Some("新名字")))
            .map(|_| ()),
    );
    knowledge
        .create(&admin(), "world", Some("她要建"))
        .expect("看得见的当已经有了");
    assert!(
        !root.account_dir(&admin()).join("kb/world").exists(),
        "她写到包里的库不另建一个盖住它"
    );

    let off = knowledge
        .rename(&admin(), "world", fields(Some(false), None))
        .expect("开关改得了");
    assert!(!off.auto);
    assert!(!knowledge.auto(&admin(), "world").expect("在"));
    let mine = std::fs::read_to_string(root.account_dir(&admin()).join("kb.toml")).expect("记下了");
    assert!(mine.contains("\"world-pack/world\" = false"), "{mine}");
    assert_eq!(
        std::fs::read_to_string(pack.join("kb/world/kb.toml")).expect("在"),
        "auto = true\n",
        "包里的不动"
    );
    knowledge
        .rename(&admin(), "world", fields(Some(true), None))
        .expect("开得回来");
    assert!(knowledge.auto(&admin(), "world").expect("在"));
    knowledge
        .reindex(&admin(), "world")
        .expect("索引是我们的，重建得了");
}

#[test]
fn your_own_base_wins_and_the_first_package_wins() {
    let scratch = Scratch::new("kb-package-names");
    let root = root_in(&scratch);
    own(&root, "world");
    let first = package(&scratch, "a-pack", "lore", "name = \"甲\"\n");
    let second = package(&scratch, "b-pack", "lore", "name = \"乙\"\n");
    let shadowed = package(&scratch, "c-pack", "world", "name = \"包里的\"\n");
    let knowledge = Knowledge::new(&root).with_packages(vec![
        ("a-pack".into(), first),
        ("b-pack".into(), second),
        ("c-pack".into(), shadowed),
    ]);
    assert_eq!(
        listed(&knowledge),
        [
            ("lore".to_string(), Some("a-pack".to_string()), false),
            ("world".to_string(), None, false),
        ]
    );
    assert_eq!(knowledge.title(&admin(), "lore").expect("在").0, "甲");
    assert!(matches!(
        knowledge.found(&admin(), "lore", fields(None, None)),
        Err(KnowledgeError::BaseExists)
    ));
}

#[test]
fn removing_a_package_takes_its_bases_index_and_switches() {
    let scratch = Scratch::new("kb-package-gone");
    let root = root_in(&scratch);
    let pack = package(&scratch, "world-pack", "world", "auto = true\n");
    own(&root, "notes");
    let knowledge = Knowledge::new(&root).with_packages(vec![("world-pack".into(), pack)]);
    knowledge.list(&admin()).expect("列得出");
    knowledge
        .rename(&admin(), "world", fields(Some(false), None))
        .expect("开关改得了");
    let index = root.index(&admin()).join("kb/packages/world-pack");
    assert!(index.is_dir());

    assert!(knowledge.follow_packages(Vec::new()).is_empty(), "都清得掉");
    assert_eq!(listed(&knowledge), [("notes".to_string(), None, false)]);
    assert!(!index.exists(), "索引跟着删");
    let mine =
        std::fs::read_to_string(root.account_dir(&admin()).join("kb.toml")).unwrap_or_default();
    assert!(!mine.contains("world-pack"), "开关跟着删：{mine}");
    assert!(
        root.index(&admin()).join("kb/notes.db").is_file(),
        "自己的不碍事"
    );
}

#[test]
fn a_package_without_kb_or_with_odd_entries_is_fine() {
    let scratch = Scratch::new("kb-package-odd");
    let root = root_in(&scratch);
    let empty = scratch.0.join("packages/empty");
    std::fs::create_dir_all(&empty).expect("建得了");
    let odd = scratch.0.join("packages/odd");
    for dir in ["kb/.hidden", "kb/Bad Name"] {
        std::fs::create_dir_all(odd.join(dir)).expect("建得了");
    }
    std::fs::write(odd.join("kb/file.md"), "不是目录\n").expect("写得进");
    let knowledge = Knowledge::new(&root).with_packages(vec![
        ("empty".into(), empty),
        ("odd".into(), odd),
        ("gone".into(), Path::new("/nowhere/at/all").to_path_buf()),
    ]);
    assert!(listed(&knowledge).is_empty());
}

/// 数据根的路上有链接（macOS 的临时目录 `/var` 就是 `/private/var` 的链接）：开关照真实的位置记、卸包时照真实的位置拿掉
/// （CI 上 macOS 报的内部错误：`miyu_fs::replace` 一层链接都不跟）。
#[cfg(unix)]
#[test]
fn switches_are_written_where_the_data_root_really_is() {
    let scratch = Scratch::new("kb-package-link");
    let real = scratch.0.join("real");
    std::fs::create_dir_all(&real).expect("建得了");
    let link = scratch.0.join("link");
    std::os::unix::fs::symlink(&real, &link).expect("链得上");
    let root = root_at(&link.join("data"));
    let pack = package(&scratch, "world-pack", "world", "auto = true\n");
    let knowledge = Knowledge::new(&root).with_packages(vec![("world-pack".into(), pack)]);
    knowledge
        .rename(&admin(), "world", fields(Some(false), None))
        .expect("开关改得了");
    assert!(!knowledge.auto(&admin(), "world").expect("在"));
    let mine = root.account_dir(&admin()).join("kb.toml");
    assert!(
        std::fs::read_to_string(&mine)
            .expect("记下了")
            .contains("world-pack/world")
    );
    assert!(knowledge.follow_packages(Vec::new()).is_empty(), "都清得掉");
    assert!(
        !std::fs::read_to_string(&mine)
            .expect("在")
            .contains("world-pack")
    );
}
