//! 收起那一行「记住」单算一类（项目主人 2026-10-17 在终端定，两个头一样）：`remember` 一次算一条，打头写「记住了 N 条」
//! （`Remembered N memories`），不打头写「N 条记忆」；排在留言之后、别的工具之前。`memory_search` 不单算，算在工具里。

use std::sync::Arc;

use miyu_kernel::testkit::{Line, Play};
use miyu_view::Projector;

use crate::support::*;

/// 收起那一行的字：每一格接起来。
fn summary(group: &serde_json::Value, key: &str) -> String {
    group[key]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|part| part["text"].as_str())
        .collect()
}

/// 她一步调 `calls` 这几件、说完：交回收起那一行，中文一份、英文一份。
fn lines(calls: &[(&str, &str)]) -> (String, String) {
    let mut stage = stage();
    stage.model([Line::calls("", calls), Line::says("好。")]);
    stage.tools(calls.iter().map(|_| Play::done("ok")));
    stage.say("hi");
    let mut projector = Projector::new(Arc::new(texts("zh")), None);
    for event in stage.log() {
        projector.event(event);
    }
    let entries = projector.entries().to_vec();
    let group = json(of_kind(&entries, "group")[0]);
    (summary(&group, "summary"), summary(&group, "summary_en"))
}

#[test]
fn remembering_counts_on_its_own() {
    let remember = ("remember", r#"{"class":"user","text":"养猫"}"#);
    let search = ("memory_search", r#"{"query":"猫"}"#);
    let (zh, en) = lines(&[remember]);
    assert!(zh.starts_with("记住了 1 条 · "), "{zh}");
    assert!(en.starts_with("Remembered 1 memory · "), "{en}");
    let (zh, en) = lines(&[remember, remember, search]);
    assert!(zh.starts_with("记住了 2 条 · 1 个工具 · "), "{zh}");
    assert!(en.starts_with("Remembered 2 memories · 1 tool · "), "{en}");
    let (zh, en) = lines(&[("shell", r#"{"command":"ls"}"#), remember]);
    assert!(
        zh.contains(" · 1 条记忆 · "),
        "不打头的照「N 条记忆」：{zh}"
    );
    assert!(en.contains(" · 1 memory · "), "{en}");
    let (_, en) = lines(&[search]);
    assert!(en.starts_with("Used 1 tool · "), "搜不单算：{en}");
}

/// 出错、被拒的没记成：同编辑，算成用过一件工具，不算记住。
#[test]
fn a_failed_remember_is_just_a_tool() {
    let mut stage = stage();
    stage.model([
        Line::calls("", &[("remember", r#"{"class":"user","text":"养猫"}"#)]),
        Line::says("好。"),
    ]);
    stage.tools([Play::Fails("too long".to_string())]);
    stage.say("hi");
    let mut projector = Projector::new(Arc::new(texts("zh")), None);
    for event in stage.log() {
        projector.event(event);
    }
    let entries = projector.entries().to_vec();
    let group = json(of_kind(&entries, "group")[0]);
    let en = summary(&group, "summary_en");
    assert!(en.starts_with("Used 1 tool · "), "{en}");
}

/// 「记住」单算以后，标题照旧接结果那一句（9-8 十补带进来的回归，网页 10-18 报：「记住 · 内容 · 记成 m14」丢了「记成」那一句）。
/// 测试用的字只认核心的说法：照一句核心的说法看接没接上。
#[test]
fn a_remember_title_still_ends_with_its_result() {
    let mut stage = stage();
    stage.model([
        Line::calls("", &[("remember", r#"{"class":"user","text":"养猫"}"#)]),
        Line::says("好。"),
    ]);
    stage.tools([Play::done("Saved as m14.")]);
    stage.say("hi");
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    for event in stage.log() {
        let mut event = event.clone();
        if let miyu_kernel::event::Body::ToolResult(result) = &mut event.body {
            result.human = Some(miyu_kernel::event::Said::new(
                "core/tool-results/cancelled-before",
            ));
        }
        projector.event(&event);
    }
    let entries = projector.entries().to_vec();
    let tool = json(of_kind(&entries, "tool")[0]);
    assert_eq!(tool["title"]["said"], "Cancelled", "{tool}");
}
