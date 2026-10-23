//! 各管各的（施工 Y-2 中上，设计 06 八又补「每人一个独立的 Miyu，共用的只有系统」）：两个账号直接调底层找的那几样，甲自己放的
//! 乙读不到；系统区的两人都读得到；同名的甲自己的盖过系统区的。人格、预设、技能、脚本、知识库各一条。

use std::fs;
use std::path::Path;

use miyu_kernel::id::AccountId;
use miyu_store::knowledge::{BaseFields, Knowledge};
use miyu_store::personas::Personas;
use miyu_store::presets::Presets;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::support::*;

fn alice() -> AccountId {
    AccountId::parse("alice").expect("合写法")
}

fn bob() -> AccountId {
    AccountId::parse("bob").expect("合写法")
}

fn put(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("有上一层")).expect("建得了");
    fs::write(path, text).expect("写得进");
}

/// 一个空的资源目录：出厂那一层什么都没有。
fn resources(scratch: &Scratch) -> ResourceRoot {
    let dir = scratch.0.join("res");
    fs::create_dir_all(&dir).expect("建得了");
    ResourceRoot::at(dir)
}

#[test]
fn personas_are_ones_own_plus_the_system_area() {
    let scratch = Scratch::new("scope-personas");
    let root = root_in(&scratch);
    let res = resources(&scratch);
    let persona =
        |dir: &Path, id: &str, text: &str| put(&dir.join(id).join("prompts/persona.md"), text);
    persona(
        &root.account_dir(&alice()).join("personas"),
        "mine",
        "alice's\n",
    );
    persona(&root.system().join("personas"), "shared", "system's\n");
    persona(&root.system().join("personas"), "same", "system's\n");
    persona(
        &root.account_dir(&alice()).join("personas"),
        "same",
        "alice's\n",
    );
    let of = |who: &AccountId| Personas::new(&res, &root, who);
    assert_eq!(
        of(&alice()).find("mine").expect("在").texts.persona,
        "alice's\n"
    );
    assert!(of(&bob()).find("mine").is_err(), "甲自己的乙读不到");
    assert_eq!(
        of(&bob()).find("shared").expect("在").texts.persona,
        "system's\n"
    );
    assert_eq!(
        of(&alice()).find("same").expect("在").texts.persona,
        "alice's\n",
        "自己的盖过系统区的"
    );
    assert_eq!(
        of(&bob()).find("same").expect("在").texts.persona,
        "system's\n"
    );
}

#[test]
fn presets_are_ones_own_plus_the_system_area() {
    let scratch = Scratch::new("scope-presets");
    let root = root_in(&scratch);
    let res = resources(&scratch);
    put(&root.account_dir(&alice()).join("presets/mine.toml"), "");
    put(&root.system().join("presets/shared.toml"), "");
    let of = |who: &AccountId| Presets::new(&res, &root, who);
    assert!(of(&alice()).find("mine").is_ok());
    assert!(of(&bob()).find("mine").is_err(), "甲自己的乙读不到");
    assert!(of(&bob()).find("shared").is_ok(), "系统区的两人都读得到");
    assert!(of(&alice()).find("shared").is_ok());
}

#[test]
fn skills_are_ones_own_then_the_system_area() {
    let scratch = Scratch::new("scope-skills");
    let root = root_in(&scratch);
    let skill = |dir: &Path, name: &str, description: &str| {
        put(
            &dir.join(name).join("SKILL.md"),
            &format!("---\nname: {name}\ndescription: {description}\n---\nbody\n"),
        );
    };
    skill(
        &miyu_store::skills::own_dir(&root, &alice()),
        "mine",
        "Alice's.",
    );
    skill(
        &miyu_store::skills::system_dir(&root),
        "shared",
        "System's.",
    );
    skill(&miyu_store::skills::system_dir(&root), "same", "System's.");
    skill(
        &miyu_store::skills::own_dir(&root, &alice()),
        "same",
        "Alice's.",
    );
    let of = |who: &AccountId| {
        miyu_store::skills::find(&[], &root, who)
            .0
            .into_iter()
            .map(|skill| (skill.head.name, skill.head.description))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        of(&alice()),
        [
            ("mine".to_string(), "Alice's.".to_string()),
            ("same".to_string(), "Alice's.".to_string()),
            ("shared".to_string(), "System's.".to_string()),
        ],
        "自己的盖过系统区同名的"
    );
    assert_eq!(
        of(&bob()),
        [
            ("same".to_string(), "System's.".to_string()),
            ("shared".to_string(), "System's.".to_string()),
        ],
        "乙只读得到系统区的"
    );
}

#[test]
fn scripts_are_ones_own_then_the_system_area() {
    let scratch = Scratch::new("scope-scripts");
    let root = root_in(&scratch);
    let script = |dir: &Path, name: &str, description: &str| {
        put(
            &dir.join(format!("{name}.sh")),
            &format!("#!/bin/sh\n# Description: {description}\necho hi\n"),
        );
    };
    script(
        &miyu_store::scripts::own_dir(&root, &alice()),
        "mine",
        "Alice's.",
    );
    script(
        &miyu_store::scripts::system_dir(&root),
        "shared",
        "System's.",
    );
    script(&miyu_store::scripts::system_dir(&root), "same", "System's.");
    script(
        &miyu_store::scripts::own_dir(&root, &alice()),
        "same",
        "Alice's.",
    );
    let of = |who: &AccountId| {
        miyu_store::scripts::find(&[], &root, who)
            .0
            .into_iter()
            .map(|script| (script.head.name, script.head.description))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        of(&alice()),
        [
            ("mine".to_string(), "Alice's.".to_string()),
            ("same".to_string(), "Alice's.".to_string()),
            ("shared".to_string(), "System's.".to_string()),
        ]
    );
    assert_eq!(
        of(&bob()),
        [
            ("same".to_string(), "System's.".to_string()),
            ("shared".to_string(), "System's.".to_string()),
        ]
    );
}

#[test]
fn a_knowledge_base_belongs_to_whoever_made_it() {
    let scratch = Scratch::new("scope-kb");
    let root: DataRoot = root_in(&scratch);
    let knowledge = Knowledge::new(&root);
    let fields = BaseFields {
        name: Some("Notes"),
        description: None,
        auto: None,
    };
    knowledge.found(&alice(), "notes", fields).expect("建得了");
    let ids = |who: &AccountId| -> Vec<String> {
        knowledge
            .list(who)
            .expect("列得出")
            .0
            .into_iter()
            .map(|base| base.id)
            .collect()
    };
    assert_eq!(ids(&alice()), ["notes"]);
    assert!(ids(&bob()).is_empty(), "甲建的库乙看不到");
}

/// 家目录那一层是别的账号的时候，系统账号的会话的记忆照旧归管理员（施工 Y-2 中上，`with_keeper`），不归那个家目录的主人。
#[test]
fn a_system_account_s_memory_still_goes_to_the_keeper() {
    let scratch = Scratch::new("scope-keeper");
    let root = root_in(&scratch);
    let res = resources(&scratch);
    let robot = AccountId::parse("robot").expect("合写法");
    let personas = Personas::new(&res, &root, &alice())
        .with_system_accounts(vec![robot.clone()])
        .with_keeper(admin());
    assert_eq!(
        personas.memory_account("nobody", &robot),
        admin(),
        "系统账号的归管理员"
    );
    assert_eq!(
        personas.memory_account("nobody", &bob()),
        bob(),
        "别的照属主"
    );
}
