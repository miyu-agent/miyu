//! 整理记忆的三种请求：认的那几句真在出厂指令里，回的照指令要的形状，别的请求不归这里。

use super::*;

/// 出厂的指令 `name`（`resources/software/memory/<name>`）。
fn instruction(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../resources/software/memory")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}：{e}", path.display()))
}

/// 一个 openai-chat 的请求体：一条 user，正文是 `text`。
fn body(text: &str) -> Vec<u8> {
    let request = json!({"model": "perf-model", "stream": true,
        "messages": [{"role": "user", "content": text}]});
    serde_json::to_vec(&request).unwrap()
}

#[test]
fn the_marks_are_in_the_factory_instructions() {
    for (name, mark) in [
        ("extract/instruction.txt", EXTRACT),
        ("tag/instruction.txt", TAG),
        ("merge/instruction.txt", MERGE),
    ] {
        assert!(instruction(name).contains(mark), "{name} 里没有「{mark}」");
    }
    // 抽取认第一轮的编号照的是一轮开头的写法。
    assert!(instruction("extract/turn-open.txt").starts_with("<turn number=\"{turn}\""));
}

#[test]
fn an_extraction_remembers_one_thing_from_the_first_turn() {
    let text = format!(
        "{EXTRACT}: …\n\n<turn number=\"17\" date=\"2026-11-22\">\nOwner: hi\n</turn>\n\n<turn number=\"23\" date=\"2026-11-22\">\n</turn>"
    );
    let answer: Value = serde_json::from_str(&super::answer(&body(&text)).unwrap()).unwrap();
    let memories = answer["memories"].as_array().unwrap();
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0]["turn"], 17);
    assert_eq!(memories[0]["class"], "user");
    // 没有一轮的抽取请求读不出来，不回。
    assert_eq!(super::answer(&body(EXTRACT)), None);
}

#[test]
fn tagging_answers_every_listed_memory() {
    let text = format!(
        "Below are memories you keep. For each one, {TAG}.\n\nMemories:\n\nm12 user 2026-11-22: a\nm7 person 2026-11-21: b\nmore words\n"
    );
    let answer: Value = serde_json::from_str(&super::answer(&body(&text)).unwrap()).unwrap();
    let ids: Vec<&str> = answer["keywords"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["m12", "m7"]);
}

#[test]
fn a_merge_changes_nothing_and_writes_a_summary() {
    let text = format!("Below are your memories. {MERGE}\n\nm3 user 2026-11-22: a");
    let answer: Value = serde_json::from_str(&super::answer(&body(&text)).unwrap()).unwrap();
    assert_eq!(answer["revised"], json!([]));
    assert_eq!(answer["retired"], json!([]));
    assert!(answer["summary"].as_str().is_some_and(|s| !s.is_empty()));
}

#[test]
fn other_requests_are_not_answered_here() {
    assert_eq!(
        super::answer(&body("perf-order-first-0: keep going.")),
        None
    );
    assert_eq!(super::answer(b"not json"), None);
    // 正文是几块的也认（有的驱动这样写）。
    let parts =
        json!({"messages": [{"role": "user", "content": [{"type": "text", "text": MERGE}]}]});
    assert!(super::answer(&serde_json::to_vec(&parts).unwrap()).is_some());
}
