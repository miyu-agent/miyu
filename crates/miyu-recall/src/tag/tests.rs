//! 写关键词交回来的读法（施工 R-15 下）：只收交进去的编号；不合的一条丢掉、别的照收；同一条写了两次照头一次；太长的截断；
//! 不是 JSON 对象、没有列表的整次算失败。

use std::collections::BTreeSet;

use super::*;
use crate::memory::KEYWORD_CHARS;

fn m(n: u64) -> MemoryId {
    MemoryId::parse(&format!("m{n}")).expect("合写法")
}

fn given(ids: &[u64]) -> BTreeSet<MemoryId> {
    ids.iter().map(|n| m(*n)).collect()
}

#[test]
fn what_was_given_is_taken_and_the_rest_dropped() {
    let answer = r#"好的：{"keywords": [
        {"id": "m1", "keywords": " 猫, 宠物 "},
        {"id": "m9", "keywords": "没交进去的"},
        {"id": "m2", "keywords": ""},
        {"id": "m3", "keywords": 7},
        {"id": "bad", "keywords": "编号写错"},
        {"id": "m1", "keywords": "第二次"},
        {"id": "m4", "keywords": "显卡,GPU"}
    ]}"#;
    assert_eq!(
        keywords(answer, &given(&[1, 2, 3, 4])),
        Ok(vec![
            (m(1), "猫, 宠物".to_string()),
            (m(4), "显卡,GPU".to_string())
        ])
    );
}

#[test]
fn too_long_keywords_are_cut() {
    let long = format!("{},尾巴", "猫".repeat(KEYWORD_CHARS - 2));
    let answer = serde_json::json!({"keywords": [{"id": "m1", "keywords": long}]}).to_string();
    assert_eq!(
        keywords(&answer, &given(&[1])),
        Ok(vec![(m(1), "猫".repeat(KEYWORD_CHARS - 2))])
    );
}

#[test]
fn an_answer_without_the_list_fails() {
    assert!(keywords("没有", &given(&[1])).is_err());
    assert!(keywords(r#"{"memories": []}"#, &given(&[1])).is_err());
    assert!(keywords(r#"{"keywords": "m1"}"#, &given(&[1])).is_err());
    assert_eq!(
        keywords(r#"{"keywords": []}"#, &given(&[1])),
        Ok(Vec::new())
    );
}
