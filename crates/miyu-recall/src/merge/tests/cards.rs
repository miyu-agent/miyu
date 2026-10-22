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
