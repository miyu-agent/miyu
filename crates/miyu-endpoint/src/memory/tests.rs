//! 核心起来时把联想要的几样交给记忆（施工 R-8，`memory.md` 第四条第 7 款）：造好的核心再交一次交不进，就是交过了。

use std::sync::Arc;

use miyu_kernel::id::AccountId;
use miyu_session::RecallTexts;
use miyu_session::testkit::Script;
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use crate::Core;
use crate::test_support::{resources, temp_root};

#[test]
fn the_core_gives_recall_its_texts_when_it_starts() {
    let (root, _dir) = temp_root("recall");
    let resources = ResourceRoot::at(resources());
    let core = Core::new(
        root,
        resources.clone(),
        Arc::new(Script::new([])),
        Catalog::default(),
        None,
        AccountId::parse("admin").expect("合写法"),
        "token".to_string(),
    );
    let again = RecallTexts::load(&resources).expect("出厂的读得出来");
    assert!(!core.memory.give_recall(again), "起来时交过了");
}

#[test]
fn clearing_says_how_many_and_one_in_the_singular() {
    // `/clear-memory` 的回执（施工 R-3 五补）：一条都没有的说没有；一条的发 `/one`，英文说单数，中文照常。
    let resources = ResourceRoot::at(resources());
    let say = |language: &str, cleared: usize| {
        let human = miyu_store::human::Human::load(&resources, language).expect("读得出");
        let said = super::slash::cleared_said(cleared);
        let fields: Vec<(&str, &str)> = said
            .fields
            .iter()
            .map(|(name, value)| (*name, value.as_str()))
            .collect();
        miyu_config::Words::sentence(&human, said.key, &fields).expect("有这一句")
    };
    assert_eq!(say("zh", 0), "这个会话没有记下的记忆。");
    assert_eq!(say("zh", 1), "清掉了这个会话记下的 1 条记忆。");
    assert_eq!(say("zh", 3), "清掉了这个会话记下的 3 条记忆。");
    assert_eq!(say("en", 1), "Cleared 1 memory this session made.");
    assert_eq!(say("en", 3), "Cleared 3 memories this session made.");
    assert_eq!(say("ja", 1), "この会話で覚えた記憶を 1 件消しました。");
}

#[test]
fn the_dream_receipt_leaves_out_zeros_and_says_how_long_it_took() {
    // `/dream` 的回执（施工 R-22 再补，2026-10-20 项目主人定）：是 0 的不写，都是 0 的说没有要改的，末尾写用时。
    let resources = ResourceRoot::at(resources());
    let say = |language: &str, dreamed: [u32; 3], summary: bool, stopped: bool, took: u64| {
        let human = miyu_store::human::Human::load(&resources, language).expect("读得出");
        let [given, revised, retired] = dreamed;
        let dreamed = miyu_session::Dreamed {
            given,
            revised,
            retired,
            summary,
            stopped,
            session: None,
        };
        crate::commands::sentence(&human, &super::slash::dreamed_said(&dreamed, took))
    };
    assert_eq!(
        say("zh", [9, 2, 0], false, false, 65_000),
        "整理完成：检查 9 条，修改 2 条，用时 1m 05s。"
    );
    assert_eq!(
        say("zh", [9, 0, 0], false, false, 12_000),
        "整理完成：检查 9 条，没有要改的，用时 12s。"
    );
    assert_eq!(
        say("zh", [9, 1, 3], true, false, 3_723_000),
        "整理完成：检查 9 条，修改 1 条，作废 3 条，摘要已更新，用时 1h 02m 03s。"
    );
    assert_eq!(
        say("zh", [4, 0, 1], true, true, 400),
        "整理已中断：检查 4 条，作废 1 条，用时 1s，其余下次再整理。",
        "叫停的不说摘要"
    );
    assert_eq!(
        say("zh", [0, 0, 0], false, false, 2_000),
        "无需整理，用时 2s。"
    );
    assert_eq!(
        say("zh", [0, 0, 0], false, true, 2_000),
        "整理已中断，用时 2s，下次再整理。"
    );
    assert_eq!(
        say("en", [9, 0, 2], false, false, 65_000),
        "Memory organized: 9 checked, 2 retired; took 1m 05s."
    );
    assert_eq!(
        say("en", [1, 1, 0], false, false, 1_000),
        "Memory organized: 1 checked, 1 revised; took 1s."
    );
    assert_eq!(
        say("zh", [1, 1, 0], false, false, 1_000),
        "整理完成：检查 1 条，修改 1 条，用时 1s。",
        "中文没有单数的那一句，退回原来的"
    );
    assert_eq!(
        say("ja", [9, 2, 0], false, false, 65_000),
        "整理完了：9 件を確認、2 件を修正。所要時間 1m 05s。"
    );
}

mod accounts;

/// 记忆、知识库算向量照系统配置和密钥文件（SU-3 上起只有这一份）：谁的那一间都取得到同一把 key，用量还记在那一间的账号名下。
#[test]
fn vectors_use_the_system_config_and_keys() {
    use miyu_config::secret::Reference;
    let (root, _dir) = temp_root("using");
    let alice = AccountId::parse("alice").expect("合写法");
    let bob = AccountId::parse("bob").expect("合写法");
    let system = root.path().join("system");
    std::fs::create_dir_all(&system).expect("建得出");
    std::fs::write(system.join("secrets.toml"), "shared = \"sk-FAKE-A\"\n").expect("写得进");
    let core = Core::new(
        root.clone(),
        ResourceRoot::at(resources()),
        Arc::new(Script::new([])),
        Catalog::default(),
        None,
        alice.clone(),
        "token".to_string(),
    )
    .with_config(crate::config::Config::load(
        &root,
        &alice,
        None,
        Vec::new(),
        crate::config::Environment::of(&[]),
    ));
    let key = |name: &str| Reference::Secret(name.to_string());
    let his = super::using_as(&core, &bob);
    assert_eq!(his.owner, bob);
    assert!(his.config.secret(&key("shared")).is_some());
    let mine = super::using_as(&core, &alice);
    assert!(mine.config.secret(&key("shared")).is_some());
    assert!(mine.config.secret(&key("missing")).is_none());
}
