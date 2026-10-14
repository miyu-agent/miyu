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
    let (root, _) = temp_root("recall");
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
