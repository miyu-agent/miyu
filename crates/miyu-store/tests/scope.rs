//! 直接调底层找的那几样只认主人家目录（施工 SU-3 下：系统区那一层拿掉）；知识库、系统账号的记忆照旧各一条（SU-3 末、SU-5 再收）。

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

/// 施工 SU-3 下：系统区放人格、预设、技能、脚本的四处不再读，只认主人家目录里的。
#[test]
fn only_the_owner_s_home_is_read_not_the_system_area() {
    let scratch = Scratch::new("scope-one-layer");
    let root = root_in(&scratch);
    let res = resources(&scratch);
    let home = root.account_dir(&alice());
    let system = root.system();
    for base in [&home, &system] {
        let name = if *base == home { "mine" } else { "shared" };
        put(
            &base.join("personas").join(name).join("prompts/persona.md"),
            "x\n",
        );
        put(&base.join("presets").join(format!("{name}.toml")), "");
        put(
            &base.join("skills").join(name).join("SKILL.md"),
            &format!("---\nname: {name}\ndescription: D.\n---\nbody\n"),
        );
        put(
            &base.join("scripts").join(format!("{name}.sh")),
            "#!/bin/sh\n# Description: D.\necho hi\n",
        );
    }
    let personas = Personas::new(&res, &root, &alice());
    assert!(personas.find("mine").is_ok());
    assert!(personas.find("shared").is_err(), "系统区的人格不读");
    let presets = Presets::new(&res, &root, &alice());
    assert!(presets.find("mine").is_ok());
    assert!(presets.find("shared").is_err(), "系统区的预设不读");
    let skills: Vec<String> = miyu_store::skills::find(&[], &root, &alice())
        .0
        .into_iter()
        .map(|skill| skill.head.name)
        .collect();
    assert_eq!(skills, ["mine"], "系统区的技能不读");
    let scripts: Vec<String> = miyu_store::scripts::find(&[], &root, &alice())
        .0
        .into_iter()
        .map(|script| script.head.name)
        .collect();
    assert_eq!(scripts, ["mine"], "系统区的脚本不读");
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
        personas.memory_account(&robot),
        admin(),
        "系统账号的归管理员"
    );
    assert_eq!(personas.memory_account(&bob()), bob(), "别的照属主");
}
