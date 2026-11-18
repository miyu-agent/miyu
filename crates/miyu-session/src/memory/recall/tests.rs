//! 联想那一块（施工 R-8）：这一段交过的照 `present` 算（模块 `memory` 两类都算，联想带过几条只数 `recall`，连同知识库自动想起
//! 的，施工 K-6 下；别的模块不算）；
//! 一块逐字节对、换行写成空格、日期照时区；超过 500 字节的那一行和以后的不加，一行都放不下的没有。

use std::path::Path;

use miyu_kernel::facts::Present;
use miyu_kernel::id::{FactKind, ModuleId, Seq};
use miyu_kernel::origin::{By, Person};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_recall::associate::Thresholds;
use miyu_recall::{Entry, MemoryId};

use super::{LIMIT, RecallTexts, render, seen};

fn texts() -> RecallTexts {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    RecallTexts::with(&resources, Thresholds::default()).expect("读得到")
}

fn present(module: &str, kind: &str, refs: &[&str]) -> Present {
    Present {
        module: ModuleId::parse(module).unwrap(),
        kind: FactKind::parse(kind).unwrap(),
        refs: refs.iter().map(ToString::to_string).collect(),
    }
}

fn entry(n: u64, text: &str) -> Entry {
    Entry {
        id: MemoryId::new(Seq::new(n).unwrap()),
        class: "user".into(),
        text: text.into(),
        sources: Vec::new(),
        audience: vec![By::Person(Person::default())],
        about: None,
        replaces: None,
        venue: None,
        // UTC 的 10 月 7 日 20 点：东九区是 8 日。
        at: Timestamp::parse("2026-10-07T20:00:00.000Z").unwrap(),
        by: By::Kernel,
        replaced_by: None,
        retired: None,
        retired_at: None,
        cleared: false,
        keywords: None,
        who: None,
        people: Vec::new(),
        place: None,
    }
}

#[test]
fn what_was_brought_is_counted_from_present() {
    let (ids, brought) = seen(&[
        present("memory", "memory", &["m1", "m2"]),
        present("memory", "recall", &["m3"]),
        present("memory", "recall", &["m4", "m5"]),
        present("roleplay", "recall", &["m9"]),
        present("knowledge", "recall", &["world/people.md#0"]),
    ]);
    let ids: Vec<String> = ids.iter().map(ToString::to_string).collect();
    assert_eq!(ids, ["m1", "m2", "m3", "m4", "m5"], "别的模块不算");
    assert_eq!(
        brought, 4,
        "数联想带过的，连同知识库自动想起带过的（施工 K-6 下，共用一份预算）"
    );
    assert_eq!(seen(&[]).1, 0);
}

#[test]
fn a_block_is_exact_and_stays_within_the_limit() {
    let east9 = UtcOffset::from_minutes(540).unwrap();
    let (a, b) = (entry(3, "用户养了一只猫\n叫团子"), entry(7, "回答要短"));
    let block = render(&texts(), &[&a, &b], east9).expect("有");
    assert_eq!(block.module.as_str(), "memory");
    assert_eq!(block.fact.kind.as_str(), "recall");
    assert_eq!(
        block.fact.text,
        "<recalled>\nm3 user 2026-10-08: 用户养了一只猫 叫团子\nm7 user 2026-10-08: 回答要短\n</recalled>\n"
    );
    assert_eq!(block.fact.refs, ["m3", "m7"]);

    let long = entry(4, &"长".repeat(140));
    let block = render(&texts(), &[&a, &long, &b], east9).expect("有");
    assert!(block.fact.text.len() <= LIMIT, "{}", block.fact.text.len());
    assert_eq!(block.fact.refs, ["m3"], "放不下的那一行和以后的都不加");

    let huge = entry(5, &"长".repeat(200));
    assert!(
        render(&texts(), &[&huge], east9).is_none(),
        "一行都放不下的没有"
    );
    assert!(render(&texts(), &[], east9).is_none());
}

/// 一行写在哪儿发生的（施工 E-4 下）：群里记的带群名，同 `memory_search` 的写法。
#[test]
fn a_line_says_where_it_was_said() {
    let east9 = UtcOffset::from_minutes(540).unwrap();
    let mut a = entry(3, "小林教用户拍日落");
    a.class = "episode".into();
    a.venue = Some(miyu_kernel::id::VenueId::parse("qq:group:7").unwrap());
    a.place = Some("周末摄影群".into());
    let block = render(&texts(), &[&a], east9).expect("有");
    assert_eq!(
        block.fact.text,
        "<recalled>\nm3 episode 2026-10-08 in QQ group 「周末摄影群」: 小林教用户拍日落\n</recalled>\n"
    );
}

/// 出厂的两张表各是各的（施工 R-14）：她主动搜的下限照 `core/memory/search.toml`，不是联想的门槛那一张。
#[test]
fn the_shipped_search_floors_come_from_their_own_table() {
    let resources = miyu_store::resources::ResourceRoot::at(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"),
    );
    let texts = RecallTexts::load(&resources).expect("出厂的读得出来");
    assert_eq!(
        texts.keyword_floor("local:granite-embedding-97m-multilingual-r2"),
        Some(0.80)
    );
    assert_eq!(texts.keyword_floor("local:bge-small-zh-v1.5"), None);
    let empty = RecallTexts::with(resources.path(), Thresholds::default()).expect("读得出来");
    assert_eq!(
        empty.keyword_floor("local:granite-embedding-97m-multilingual-r2"),
        None,
        "with 造的是空表"
    );
}

/// 搜记下的时只照意思找到的那一路照 `core/memory/near.toml`（施工 R-24），不是搜以前的对话那一张；表里没有的、`with` 造的空表
/// 照 [`miyu_recall::FLOOR`]。
#[test]
fn the_shipped_near_floor_comes_from_its_own_table() {
    let resources = miyu_store::resources::ResourceRoot::at(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"),
    );
    let granite = "local:granite-embedding-97m-multilingual-r2";
    let texts = RecallTexts::load(&resources).expect("出厂的读得出来");
    assert_eq!(texts.near_floor(granite), 0.80);
    assert_eq!(texts.turns_floor(granite), 0.84, "两张表各是各的");
    assert_eq!(texts.near_floor("local:other"), miyu_recall::FLOOR);
    let empty = RecallTexts::with(resources.path(), Thresholds::default()).expect("读得出来");
    assert_eq!(
        empty.near_floor(granite),
        miyu_recall::FLOOR,
        "with 造的是空表"
    );
}
