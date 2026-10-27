//! 搜人卡（施工 E-3）：名字、外号、平台号都算关键词，对上是谁的只给他，没对上的照正文里少见的词找。名字、编号是虚构的。

use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;

use super::*;

fn at(minutes: i64) -> Timestamp {
    let start = Timestamp::parse("2026-10-27T08:00:00.000Z").expect("合写法");
    Timestamp::from_unix_millis(start.unix_millis() + minutes * 60_000).expect("合写法")
}

fn card(who: &str, names: &[&str], text: &str, minutes: i64) -> PersonCard {
    PersonCard {
        who: who.to_string(),
        text: text.to_string(),
        names: names.iter().map(ToString::to_string).collect(),
        at: at(minutes),
        by: By::Kernel,
        retired: None,
    }
}

fn whos(found: &[&PersonCard]) -> Vec<String> {
    found.iter().map(|card| card.who.clone()).collect()
}

fn three() -> Vec<PersonCard> {
    vec![
        card(
            "qq:20001",
            &["小林", "林林"],
            "小林是群里认识的摄影爱好者，周末常去海边拍照。",
            1,
        ),
        card(
            "qq:20002",
            &["GNU/Rust++"],
            "写 Rust 的群友，在群里认识的，下周搬去大阪。",
            2,
        ),
        card(
            "qq:20003",
            &["夏夏"],
            "喜欢猫，养了一只橘猫，群里认识的。",
            3,
        ),
    ]
}

/// 验收的核心：问「你认识小林吗」整句，对上叫法「小林」，只给小林的卡（别人卡里的「认识」不混进来）。
#[test]
fn a_whole_question_finds_the_card_by_a_name() {
    let cards = three();
    assert_eq!(whos(&find_cards("你认识小林吗", &cards, 3)), ["qq:20001"]);
    assert_eq!(whos(&find_cards("林林最近怎么样", &cards, 3)), ["qq:20001"]);
    assert_eq!(
        whos(&find_cards("小林喜欢猫吗", &cards, 3)),
        ["qq:20001"],
        "夏夏的卡里有「喜欢」，问的是小林就不给"
    );
}

/// 叫法比两个字长的：句子里只说了其中两个字也对得上（两字词落在叫法里）；英文的叫法照词对，不分大小写。
#[test]
fn part_of_a_longer_name_and_an_english_word_find_it() {
    let cards = vec![
        card("qq:30001", &["林夏晴"], "大学同学。", 1),
        three().remove(1),
    ];
    assert_eq!(whos(&find_cards("林夏最近还好吗", &cards, 3)), ["qq:30001"]);
    assert_eq!(
        whos(&find_cards("do you know rust guy", &cards, 3)),
        ["qq:20002"]
    );
    assert_eq!(
        whos(&find_cards("GNU/RUST++ 是谁", &cards, 3)),
        ["qq:20002"]
    );
}

/// 平台编号整个说出来的、只说号码的，都对得上。
#[test]
fn the_platform_id_finds_it() {
    let cards = three();
    assert_eq!(whos(&find_cards("qq:20003 是谁", &cards, 3)), ["qq:20003"]);
    assert_eq!(
        whos(&find_cards("20003 这个号你认识吗", &cards, 3)),
        ["qq:20003"]
    );
    assert!(
        find_cards("2000 是谁", &cards, 3).is_empty(),
        "号码的一截不算"
    );
}

/// 一个字的叫法不拿来整个对（太容易碰上），句子里的一个字也不拿来对叫法。
#[test]
fn a_single_character_does_not_match() {
    let cards = vec![card("qq:40001", &["败"], "爱打牌。", 1)];
    assert!(find_cards("我又败了", &cards, 3).is_empty());
    let cards = vec![card("qq:40002", &["小林"], "爱打牌。", 1)];
    assert!(find_cards("林", &cards, 3).is_empty());
}

/// 没对上是谁的照正文找：少见的词对得上的给，越多越前；一半以上的卡都有的词（「认识」「群里」）不算。
#[test]
fn without_a_name_the_text_finds_it() {
    let cards = three();
    assert_eq!(whos(&find_cards("谁常去海边拍照", &cards, 3)), ["qq:20001"]);
    assert_eq!(whos(&find_cards("谁搬去大阪了", &cards, 3)), ["qq:20002"]);
    assert!(
        find_cards("谁喜欢拍照", &cards, 3).is_empty(),
        "各只对上一个词（「喜欢」「拍照」）的不算"
    );
    assert!(
        find_cards("群里认识的人", &cards, 3).is_empty(),
        "谁的卡都写的词不算"
    );
    assert_eq!(
        whos(&find_cards(
            "海边拍照的摄影爱好者，还有养了一只橘猫的",
            &cards,
            3
        )),
        ["qq:20001", "qq:20003"],
        "对上的词多的在前"
    );
}

/// 只有一张卡的，正文里连成一截的词照样找得到（不被「一半以上」挡掉）；和他不相干的话不出，零散对上两个常用词（「喜欢」
/// 「什么」）的也不出。
#[test]
fn a_single_card_is_found_by_its_text_and_only_then() {
    let cards = vec![three().remove(0)];
    assert_eq!(whos(&find_cards("谁常去海边拍照", &cards, 3)), ["qq:20001"]);
    assert!(find_cards("你喜欢什么颜色", &cards, 3).is_empty());
    assert!(find_cards("今天天气怎么样", &cards, 3).is_empty());
    let cards = vec![card("qq:40003", &["阿明"], "他喜欢打牌，什么局都去。", 1)];
    assert!(
        find_cards("主人喜欢什么", &cards, 3).is_empty(),
        "「喜欢」「什么」都对上，可没连成一截、没有专名"
    );
    assert_eq!(
        whos(&find_cards("谁喜欢打牌", &cards, 3)),
        ["qq:40003"],
        "「喜欢打牌」连成一截"
    );
}

/// 正文里有英文专名和另一个词的，照正文找得到：「群里写 Rust 的那个是谁」（「群里」谁的卡都写，不算）。
#[test]
fn a_name_like_word_in_the_text_finds_it() {
    let mut cards = three();
    cards.remove(1);
    cards.push(card(
        "qq:20005",
        &["老周"],
        "老周在群里写 Rust，下周搬去大阪。",
        4,
    ));
    assert_eq!(
        whos(&find_cards("群里写 Rust 的那个是谁", &cards, 3)),
        ["qq:20005"]
    );
    assert!(
        find_cards("谁会 Rust", &cards, 3).is_empty(),
        "只对上一个词的不算"
    );
}

/// 一句话的长叫法（群名片常这样）：只认整个出现、英文数字词，不拿两字词去落，「我不是这个意思」「你今天开心吗」不出卡。
#[test]
fn a_long_name_is_not_hit_by_a_word_of_it() {
    let cards = vec![
        card("qq:60001", &["我不是MoYingJi"], "爱发表情包。", 1),
        card("qq:60002", &["本群最大baka"], "常在群里刷屏。", 2),
        card("qq:60003", &["今天也要开心"], "早上总发早安。", 3),
        three().remove(0),
    ];
    for said in [
        "我不是这个意思",
        "你今天开心吗",
        "本群最近好热闹",
        "今天也要加油",
    ] {
        assert!(find_cards(said, &cards, 3).is_empty(), "{said}");
    }
    assert_eq!(whos(&find_cards("你认识小林吗", &cards, 3)), ["qq:20001"]);
    assert_eq!(whos(&find_cards("moyingji 在吗", &cards, 3)), ["qq:60001"]);
    assert_eq!(
        whos(&find_cards("「今天也要开心」是谁", &cards, 3)),
        ["qq:60003"]
    );
    assert_eq!(whos(&find_cards("baka 是谁", &cards, 3)), ["qq:60002"]);
}

/// 几张都对上的：对上的那截长的在前，编号整个说出来的最先，一样的新写的在前；最多 `limit` 张。
#[test]
fn the_longer_match_comes_first_and_the_limit_holds() {
    let cards = vec![
        card("qq:50001", &["小林"], "甲。", 1),
        card("qq:50002", &["小林子"], "乙。", 2),
        card("qq:50003", &["阿林"], "丙。", 3),
        card("qq:50004", &["小林"], "丁。", 4),
    ];
    assert_eq!(
        whos(&find_cards("小林子和小林", &cards, 3)),
        ["qq:50002", "qq:50004", "qq:50001"]
    );
    assert_eq!(
        whos(&find_cards("qq:50003 和小林子", &cards, 1)),
        ["qq:50003"]
    );
}

/// 什么都没对上的、没有卡的、切不出词的：空的。
#[test]
fn nothing_matches_nothing() {
    let cards = three();
    assert!(find_cards("今天天气不错", &cards, 3).is_empty());
    assert!(find_cards("你认识小林吗", &[], 3).is_empty());
    assert!(find_cards("？！", &cards, 3).is_empty());
}
