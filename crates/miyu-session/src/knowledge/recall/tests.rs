//! 自动想起那一块（施工 K-6 下）：这一段交过的照 `present` 算（模块 `knowledge` 的才算交过，带过几条连同记忆联想的一起数）；
//! 一块逐字节对，PDF 的说第几页、没标题的只写路径，长的截到 120 个字接省略号；超过 600 字节的那一段和以后的不加。

use miyu_kernel::facts::Present;
use miyu_kernel::id::{FactKind, ModuleId};
use miyu_kernel::time::Timestamp;
use miyu_store::knowledge::Hit;
use miyu_store::resources::ResourceRoot;

use super::{LIMIT, load, named, render, room, seen};

fn present(module: &str, kind: &str, refs: &[&str]) -> Present {
    Present {
        module: ModuleId::parse(module).unwrap(),
        kind: FactKind::parse(kind).unwrap(),
        refs: refs.iter().map(ToString::to_string).collect(),
    }
}

fn hit(path: &str, chunk: usize, heading: &str, page: Option<usize>, text: &str) -> Hit {
    Hit {
        kb: "world".to_string(),
        path: path.to_string(),
        chunk,
        heading: heading.to_string(),
        page,
        text: text.to_string(),
        at: Timestamp::parse("2026-10-13T00:00:00.000Z").unwrap(),
    }
}

fn texts() -> super::RecallTexts {
    let resources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    load(&ResourceRoot::at(resources)).expect("读得到")
}

#[test]
fn what_was_brought_is_counted_with_the_memory_recall() {
    let (seen, brought) = seen(&[
        present("knowledge", "recall", &["world/people.md#0"]),
        present(
            "knowledge",
            "recall",
            &["world/places.md#1", "world/lore.md#0"],
        ),
        present("memory", "recall", &["m3", "m4"]),
        present("memory", "memory", &["m1"]),
        present("roleplay", "recall", &["x"]),
    ]);
    assert_eq!(
        seen.into_iter().collect::<Vec<_>>(),
        ["world/lore.md#0", "world/people.md#0", "world/places.md#1"],
        "只有知识库的算交过"
    );
    assert_eq!(brought, 5, "联想、自动想起合起来数");
}

#[test]
fn a_block_is_exact_and_stays_within_the_limit() {
    let named = hit(
        "people.md",
        0,
        "人物 › 艾琳",
        None,
        "守灯人，十九岁。\n最怕打雷。",
    );
    let pdf = hit("manual.pdf", 2, "", Some(3), "每年十月打疫苗。");
    let plain = hit("notes.txt", 0, "", None, "东边的镇子靠海。");
    let block = render(&texts(), &[&named, &pdf, &plain]).expect("有");
    assert_eq!(block.module.as_str(), "knowledge");
    assert_eq!(block.fact.kind.as_str(), "recall");
    assert_eq!(
        block.fact.text,
        "<knowledge>\n[world] people.md: 人物 › 艾琳\n守灯人，十九岁。 最怕打雷。\n[world] manual.pdf, page 3\n每年十月打疫苗。\n[world] notes.txt\n东边的镇子靠海。\n</knowledge>\n"
    );
    assert_eq!(
        block.fact.refs,
        [
            "world/people.md#0",
            "world/manual.pdf#2",
            "world/notes.txt#0"
        ]
    );

    let long = hit("lore.md", 1, "规矩 › 潮汐魔法", None, &"潮".repeat(130));
    let block = render(&texts(), &[&long]).expect("有");
    assert!(
        block
            .fact
            .text
            .contains(&format!("\n{}…\n", "潮".repeat(120))),
        "截到 120 个字接省略号：{}",
        block.fact.text
    );
    let block = render(&texts(), &[&long, &long, &named]).expect("有");
    assert!(block.fact.text.len() <= LIMIT, "{}", block.fact.text.len());
    assert_eq!(block.fact.refs.len(), 1, "放不下的那一段和以后的都不加");
    assert!(render(&texts(), &[]).is_none());
}

#[test]
fn names_need_two_characters_skip_what_was_brought_and_stop_at_the_room() {
    let sections = [
        hit("people.md", 0, "人物 › 艾琳", None, "守灯人。"),
        hit("people.md", 1, "人物 › 巴德", None, "老船长。"),
        hit("people.md", 2, "人物 › 灯", None, "一个字的标题。"),
        hit("people.md", 3, "人物 › 老莫", None, "杂货铺。"),
        hit("people.md", 4, "人物 › 小雀", None, "孤儿。"),
        hit("manual.pdf", 0, "", Some(1), "没有标题的一页。"),
    ];
    let keys = texts().keys;
    let none = std::collections::BTreeSet::new();
    assert_eq!(
        named(&sections, "灯又灭了，艾琳在吗", &none, 3, &keys),
        ["world/people.md#0"],
        "一个字的标题不算"
    );
    let all = "艾琳、巴德、老莫、小雀都来了";
    assert_eq!(
        named(&sections, all, &none, 3, &keys),
        [
            "world/people.md#0",
            "world/people.md#1",
            "world/people.md#3"
        ],
        "照索引里的先后，最多 room 段"
    );
    let brought: std::collections::BTreeSet<String> = ["world/people.md#0".to_string()].into();
    assert_eq!(
        named(&sections, all, &brought, 2, &keys),
        ["world/people.md#1", "world/people.md#3"],
        "交过的不要"
    );
    assert!(named(&sections, "今天好累啊", &none, 3, &keys).is_empty());
}

#[test]
fn the_room_is_three_a_turn_within_forty_a_context() {
    assert_eq!(room(0), 3);
    assert_eq!(room(38), 2);
    assert_eq!(room(40), 0);
    assert_eq!(room(45), 0);
}

#[test]
fn keywords_under_the_heading_bring_their_section_like_its_name() {
    // 施工 K-9：标题下一行写的关键词当别名，一节切成几块的每块都算；两个字以上、不分大小写，标题名也不分；没有标题的段写了
    // 也不算；只认头一行。
    let keys = texts().keys;
    let sections = [
        hit(
            "people.md",
            0,
            "人物 › 艾琳",
            None,
            "关键词：守灯人、灯塔的姑娘、Eileen、雷\n\n守灯人，十九岁。",
        ),
        hit("people.md", 1, "人物 › 艾琳", None, "最怕打雷。"),
        hit(
            "people.md",
            2,
            "人物 › Bard",
            None,
            "老船长。\n关键词：船长",
        ),
        hit("notes.txt", 0, "", None, "关键词：杂货铺\n东边的镇子。"),
    ];
    let none = std::collections::BTreeSet::new();
    assert_eq!(
        named(&sections, "灯塔的姑娘最近怎么样", &none, 3, &keys),
        ["world/people.md#0", "world/people.md#1"],
        "一节的每一块都照这一节的关键词算"
    );
    assert_eq!(
        named(&sections, "eileen and BARD", &none, 3, &keys),
        [
            "world/people.md#0",
            "world/people.md#1",
            "world/people.md#2"
        ],
        "英文的关键词、标题名都不分大小写"
    );
    assert!(
        named(&sections, "打雷了", &none, 3, &keys).is_empty(),
        "一个字的关键词不算"
    );
    assert!(
        named(&sections, "老船长", &none, 3, &keys).is_empty(),
        "不是头一行的不是关键词"
    );
    assert!(
        named(&sections, "去杂货铺", &none, 3, &keys).is_empty(),
        "没有标题的段没有关键词"
    );
}

#[test]
fn the_keyword_line_is_left_out_of_the_block() {
    let named = hit(
        "people.md",
        0,
        "人物 › 艾琳",
        None,
        "关键词：守灯人、Eileen\n\n守灯人，十九岁。",
    );
    let plain = hit("notes.txt", 0, "", None, "关键词：杂货铺\n东边的镇子。");
    let block = render(&texts(), &[&named, &plain]).expect("有");
    assert_eq!(
        block.fact.text,
        "<knowledge>\n[world] people.md: 人物 › 艾琳\n守灯人，十九岁。\n[world] notes.txt\n关键词：杂货铺 东边的镇子。\n</knowledge>\n",
        "有标题的去掉关键词那一行，没有标题的原样"
    );
}
