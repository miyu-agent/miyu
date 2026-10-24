//! 每人一份装着的包（施工 U23 上下）：成员手放进自己家目录那一层的包只有他看得到（成员现在装不了包，所以手放、直接调）；系统那一层
//! 重读以后他那份跟着换；投影里技能的显示名照会话属主找。

use std::path::Path;
use std::sync::Arc;

use miyu_kernel::id::AccountId;
use miyu_session::testkit::Script;
use miyu_store::packages::Layer;
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use crate::Core;
use crate::test_support::{resources, temp_root};

/// 在 `dir` 下面放一个只带一张 `[package]` 的包 `id`，带一个技能 `skill`（显示名 `title`）。
fn put(dir: &Path, id: &str, skill: &str, title: &str) {
    let folder = dir.join(id);
    std::fs::create_dir_all(folder.join("skills").join(skill)).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        format!("[package]\nname = {{ en = \"{id}\" }}\n"),
    )
    .expect("写得进");
    std::fs::write(
        folder.join("skills").join(skill).join("SKILL.md"),
        format!("---\nname: {skill}\ndescription: Do {skill}.\nmetadata:\n  display_name: {title}\n---\n\nBody.\n"),
    )
    .expect("写得进");
}

/// 机主 alice 的核心。
fn core(root: &miyu_store::root::DataRoot) -> Arc<Core> {
    Arc::new(Core::new(
        root.clone(),
        ResourceRoot::at(resources()),
        Arc::new(Script::new([])),
        Catalog::default(),
        None,
        AccountId::parse("alice").expect("合写法"),
        "token".to_string(),
    ))
}

/// `found` 里有没有 `layer` 那一层读成了的 `id`。
fn has(found: &[miyu_store::packages::Found], id: &str, layer: Layer) -> bool {
    found
        .iter()
        .any(|one| one.id == id && one.layer == layer && one.read.is_ok())
}

#[test]
fn an_account_s_own_layer_is_seen_only_by_it_and_follows_the_system_one() {
    let (root, dir) = temp_root("own-layer");
    let (alice, bob) = (
        AccountId::parse("alice").expect("合写法"),
        AccountId::parse("bob").expect("合写法"),
    );
    let own = root.account_dir(&bob).join("packages");
    put(&own, "kit", "bob-notes", "Bob notes");
    // 他自己放的技能（不在包里）：投影照属主的家目录找。
    let mine = root.account_dir(&bob).join("skills/own-notes/SKILL.md");
    std::fs::create_dir_all(mine.parent().expect("有上级")).expect("建得了");
    std::fs::write(
        &mine,
        "---\nname: own-notes\ndescription: Do it.\nmetadata:\n  display_name: Own notes\n---\n\nBody.\n",
    )
    .expect("写得进");
    let core = core(&root);
    assert!(
        has(&core.packages_of(&bob), "kit", Layer::User),
        "他看得到自己那层的"
    );
    assert!(
        !has(&core.packages_of(&alice), "kit", Layer::User),
        "别人看不到"
    );
    assert!(
        Arc::ptr_eq(&core.packages_of(&alice), &core.system_packages()),
        "自己那层空的就是系统那一份"
    );
    assert!(
        !has(&core.system_packages(), "kit", Layer::User),
        "系统那一份没有"
    );
    assert!(
        crate::packages::folders(&core, &bob).contains(&own.join("kit")),
        "人格、预设照他看得到的包找"
    );
    assert!(
        !crate::packages::folders(&core, &alice).contains(&own.join("kit")),
        "别人的不找"
    );
    let installed = crate::packages::installed(&core, &bob);
    assert!(
        has(&installed(), "kit", Layer::User),
        "交给他的会话的是他看得到的"
    );
    // 用到时造了留着：再放一个，系统那一层没变就还是那一份；系统那一层重读以后跟着换。
    put(&own, "more", "more-notes", "More notes");
    assert!(
        !has(&core.packages_of(&bob), "more", Layer::User),
        "留着的那一份"
    );
    core.reload_packages();
    assert!(
        core.own_packages
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty(),
        "装卸以后留着的都清掉，不再用的人不占着"
    );
    assert!(
        has(&core.packages_of(&bob), "more", Layer::User),
        "系统那一层重读以后重造"
    );
    // 系统那一份换了、没经清掉（正读着的时候装卸了）：取的时候照比，当场重造。
    put(&own, "late", "late-notes", "Late notes");
    assert!(
        !has(&core.packages_of(&bob), "late", Layer::User),
        "留着的那一份"
    );
    let swapped = Arc::new(core.system_packages().to_vec());
    *core
        .packages
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = swapped;
    assert!(
        has(&core.packages_of(&bob), "late", Layer::User),
        "系统那一份换了的当场重造"
    );
    let titles = |account: &AccountId| {
        crate::view::texts(
            &core.resources,
            "en",
            &crate::view::Titles::of(&core, account),
        )
        .expect("读得成")
        .skills
    };
    assert_eq!(
        titles(&bob).get("bob-notes").map(String::as_str),
        Some("Bob notes"),
        "投影照属主找他自己那层的技能"
    );
    assert_eq!(titles(&alice).get("bob-notes"), None, "别人的会话找不到");
    assert_eq!(
        titles(&bob).get("own-notes").map(String::as_str),
        Some("Own notes"),
        "他自己放的照他的家目录找"
    );
    assert_eq!(titles(&alice).get("own-notes"), None);
    drop(core);
    if let Err(error) = std::fs::remove_dir_all(&dir) {
        eprintln!("临时目录没删掉：{error}");
    }
}
