//! 抽取交回来的怎么读：前后多的话不管、零条、几条、一条不合的丢掉别的照收、读不成的整次失败。

use super::*;

fn turns() -> BTreeSet<u64> {
    [5, 9].into_iter().collect()
}

#[test]
fn the_object_is_found_inside_extra_words_and_fences() {
    let answer = "Here you go:\n```json\n{\"memories\": [{\"class\": \"user\", \"text\": \" 用户养了一只猫 \", \"turn\": 5, \"about\": \"2026-10-09\"}]}\n```";
    assert_eq!(
        candidates(answer, &turns(), 120).expect("读得成"),
        [Candidate {
            class: "user".into(),
            text: "用户养了一只猫".into(),
            turn: 5,
            about: Some("2026-10-09".into()),
            who: None,
        }]
    );
    assert_eq!(
        candidates(r#"{"memories": []}"#, &turns(), 120).expect("零条是常态"),
        []
    );
}

#[test]
fn a_bad_entry_is_dropped_and_the_rest_kept() {
    let long = "长".repeat(121);
    let answer = format!(
        r#"{{"memories": [
            {{"class": "user", "text": "好的一条", "turn": 9}},
            {{"class": "project", "text": "不认识的类", "turn": 9}},
            {{"class": "feedback", "text": "  ", "turn": 9}},
            {{"class": "feedback", "text": "{long}", "turn": 9}},
            {{"class": "episode", "text": "不在这一段", "turn": 7}},
            {{"class": "episode", "text": "没有 turn"}},
            {{"class": "reference", "text": "日子写错了", "turn": 5, "about": "下周三"}},
            "不是对象"
        ]}}"#
    );
    let read = candidates(&answer, &turns(), 120).expect("读得成");
    assert_eq!(
        read.iter().map(|one| one.text.as_str()).collect::<Vec<_>>(),
        ["好的一条", "日子写错了"]
    );
    assert_eq!(read[1].about, None, "日子写错的当没写，这一条照收");
    assert!(
        candidates(
            &format!(
                r#"{{"memories":[{{"class":"user","text":"{}","turn":5}}]}}"#,
                "长".repeat(120)
            ),
            &turns(),
            120
        )
        .expect("读得成")
        .len()
            == 1,
        "正好 120 个字收"
    );
}

#[test]
fn an_unreadable_answer_fails_as_a_whole() {
    for answer in [
        "没有对象",
        "} {",
        r#"{"memories": "x"}"#,
        r#"{"other": []}"#,
        "{not json}",
    ] {
        assert!(candidates(answer, &turns(), 120).is_err(), "{answer}");
    }
}

/// 认识的人那一类带是谁（施工 E-1 上）：去掉前后空白，空的当没写；别的类写了也不收。
#[test]
fn a_person_says_who_and_other_classes_do_not() {
    let answer = r#"{"memories": [
        {"class": "person", "text": "小林用 RX 7800 XT", "turn": 5, "who": " 小林 "},
        {"class": "person", "text": "没说是谁", "turn": 5, "who": "  "},
        {"class": "user", "text": "主人养猫", "turn": 9, "who": "小林"}
    ]}"#;
    let who: Vec<(String, Option<String>)> = candidates(answer, &turns(), 120)
        .expect("读得成")
        .into_iter()
        .map(|candidate| (candidate.class, candidate.who))
        .collect();
    assert_eq!(
        who,
        [
            ("person".to_string(), Some("小林".to_string())),
            ("person".to_string(), None),
            ("user".to_string(), None),
        ]
    );
}
