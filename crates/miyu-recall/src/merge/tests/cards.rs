//! 人卡怎么读（施工 E-1 下）：只收这一批里出现过的人、一个人照头一张、正文截在句子上、叫法去空白去重有上限；没写的当空的，
//! 写了不是列表的整次失败。

use super::*;

fn people() -> BTreeSet<String> {
    ["qq:10086", "小林"].into_iter().map(String::from).collect()
}

#[test]
fn only_cards_of_people_in_this_batch_are_taken() {
    let answer = r#"Here: {"revised": [], "cards": [
        {"who": " qq:10086 ", "text": " 在 Rust 群认识的，用 RX 7800 XT。 ", "names": [" GNU/Rust++ ", "", "GNU/Rust++", 3]},
        {"who": "qq:10086", "text": "第二张不要"},
        {"who": "qq:999", "text": "这一批里没有他"},
        {"who": "小林", "text": "  "},
        {"text": "没写是谁"}
    ]}"#;
    let read = cards(answer, &people()).expect("读得成");
    assert_eq!(
        read,
        vec![Drafted {
            who: "qq:10086".into(),
            text: "在 Rust 群认识的，用 RX 7800 XT。".into(),
            names: vec!["GNU/Rust++".into()],
        }]
    );
}

#[test]
fn a_card_holds_four_hundred_characters() {
    // 设计 42 PN4（2026-10-26 项目主人改成 400）：熟人的印象 200～400 字，400 字的整张收，第 401 个字截掉。
    let card = |text: String| {
        let answer = serde_json::json!({"cards": [{"who": "小林", "text": text}]}).to_string();
        cards(&answer, &people()).expect("读得成")[0].text.clone()
    };
    assert_eq!(card("印".repeat(400)), "印".repeat(400));
    assert_eq!(card("印".repeat(401)), "印".repeat(400));
}

#[test]
fn a_long_card_is_cut_at_a_sentence_and_names_are_capped() {
    let long = format!("{}。{}", "认".repeat(CARD_CHARS - 10), "多".repeat(40));
    let many: Vec<String> = (0..NAMES + 3).map(|n| format!("名{n}")).collect();
    let answer = serde_json::json!({
        "cards": [{"who": "小林", "text": long, "names": many, }]
    })
    .to_string();
    let read = cards(&answer, &people()).expect("读得成");
    assert_eq!(read[0].text, format!("{}。", "认".repeat(CARD_CHARS - 10)));
    assert_eq!(read[0].names.len(), NAMES);
    let too_long = "长".repeat(NAME_CHARS + 1);
    let answer = serde_json::json!({"cards": [{"who": "小林", "text": "同学。", "names": [too_long, "林林"]}]})
        .to_string();
    assert_eq!(
        cards(&answer, &people()).expect("读得成")[0].names,
        vec!["林林".to_string()]
    );
}

#[test]
fn no_cards_is_empty_and_a_wrong_shape_fails() {
    assert_eq!(cards(r#"{"revised": []}"#, &people()), Ok(Vec::new()));
    assert!(cards(r#"{"cards": "小林是同学"}"#, &people()).is_err());
    assert!(cards("no object", &people()).is_err());
}

#[test]
fn names_put_the_new_first_and_skip_the_id_itself() {
    let known = vec![
        "阿林".to_string(),
        "qq:10086".to_string(),
        "林林".to_string(),
    ];
    assert_eq!(
        names("qq:10086", &["林林".to_string()], &known),
        vec!["林林".to_string(), "阿林".to_string()]
    );
}

/// 只有名字的人前面多一截的照认（`person:老王`），平台编号不这么认，去掉一截也不在这一批里的照拦。
#[test]
fn a_name_with_a_prefix_is_still_that_person() {
    let people: BTreeSet<String> = ["qq:2", "老王"].into_iter().map(String::from).collect();
    let answer = r#"{"cards": [
        {"who": "person:老王", "text": "大学室友。"},
        {"who": "person:qq:2", "text": "平台编号不这么认"},
        {"who": "phone:13800001234", "text": "去掉一截也不在这一批里"}
    ]}"#;
    let read = cards(answer, &people).expect("读得成");
    assert_eq!(
        read.iter()
            .map(|card| card.who.as_str())
            .collect::<Vec<_>>(),
        vec!["老王"]
    );
}

#[test]
fn only_their_own_id_is_taken_out_of_the_text() {
    // 施工 E-1 修二：实测卡的正文里又写了一遍他自己的「qq:号」，编号在「是谁」那一格，正文里是重复。施工 E-2：只去他自己的，
    // 他的小号、别人的编号、手机号是联系方式，照留（项目主人 2026-10-26 问过为什么卡里不写 QQ 号）。
    let answer = r#"{"cards": [{"who": "qq:10086", "text": "qq:10086 说过想去大阪（qq:10086），小号 qq:2233，和 wx:42 (qq:7) 是朋友，手机 13800001234。"}]}"#;
    let read = cards(answer, &people()).expect("读得成");
    assert_eq!(
        read[0].text,
        "说过想去大阪，小号 qq:2233，和 wx:42 (qq:7) 是朋友，手机 13800001234。"
    );
    // 前面、后面紧挨着别的字母数字的不是他的编号：qq:100861、xqq:10086 照留。
    assert_eq!(
        without_id("qq:100861 和 xqq:10086", "qq:10086"),
        "qq:100861 和 xqq:10086"
    );
    // 不是编号的照留：数字开头的、大写开头的、后面紧挨着字母的。
    assert_eq!(
        without_id("时间 10:30，QQ:1 和 a:1x", "a:1"),
        "时间 10:30，QQ:1 和 a:1x"
    );
    // 只有名字的人没有编号，什么都不去。
    assert_eq!(without_id("老王 qq:10086", "老王"), "老王 qq:10086");
}

#[test]
fn only_names_seen_in_the_sources_are_kept_and_the_used_ones_come_first() {
    // 施工 E-1 修二：实测交回的叫法里有源消息里从没出现过的（编的），有的又空着、少了改名以前的。
    let used = vec!["林林子".to_string(), "小林".to_string()];
    let drafted = vec![
        "小林花".to_string(),
        "林林".to_string(),
        "小林".to_string(),
        "qq:10086".to_string(),
    ];
    let seen = |name: &str| "大家都叫他林林".contains(name);
    assert_eq!(
        kept_names("qq:10086", &used, &drafted, seen),
        vec!["林林子".to_string(), "小林".to_string(), "林林".to_string()]
    );
    assert_eq!(
        kept_names("qq:10086", &used, &[], |_| false),
        vec!["林林子".to_string(), "小林".to_string()]
    );
}
