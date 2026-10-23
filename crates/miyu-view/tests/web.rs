//! 网络搜索、抓网页（项目主人 2026-10-19 在终端定，两个头一样）：收起那一行单算两类，排在记住之后、别的工具之前——搜索打头写
//! 「搜索了 N 次网络」（`Searched the web N times`），不打头写「N 次搜索」；抓取打头写「抓取了 N 个网页」（`Fetched N pages`），
//! 不打头写「N 个网页」；出错的算工具。搜索的来源挑到标题的 `via`，结果那一句不再带它。

use std::sync::Arc;

use miyu_kernel::event::{Body, Said};
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

/// 她一步调 `calls` 这几件，照 `plays` 回、说完：交回收起那一行，中文一份、英文一份。
fn lines(calls: &[(&str, &str)], plays: Vec<Play>) -> (String, String) {
    let mut stage = stage();
    stage.model([Line::calls("", calls), Line::says("好。")]);
    stage.tools(plays);
    stage.say("hi");
    let mut projector = Projector::new(Arc::new(texts("zh")), None);
    for event in stage.log() {
        projector.event(event);
    }
    let entries = projector.entries().to_vec();
    let group = json(of_kind(&entries, "group")[0]);
    (summary(&group, "summary"), summary(&group, "summary_en"))
}

fn done(n: usize) -> Vec<Play> {
    (0..n).map(|_| Play::done("ok")).collect()
}

const SEARCH: (&str, &str) = ("web_search", r#"{"query":"rust"}"#);
const FETCH: (&str, &str) = ("web_fetch", r#"{"url":"https://example.invalid/"}"#);

#[test]
fn searching_and_fetching_count_on_their_own() {
    let (zh, en) = lines(&[SEARCH], done(1));
    assert!(zh.starts_with("搜索了 1 次网络 · "), "{zh}");
    assert!(en.starts_with("Searched the web 1 time · "), "{en}");
    let (zh, en) = lines(&[SEARCH, SEARCH, FETCH], done(3));
    assert!(zh.starts_with("搜索了 2 次网络 · 1 个网页 · "), "{zh}");
    assert!(
        en.starts_with("Searched the web 2 times · 1 page · "),
        "{en}"
    );
    let (zh, en) = lines(&[FETCH, FETCH], done(2));
    assert!(zh.starts_with("抓取了 2 个网页 · "), "{zh}");
    assert!(en.starts_with("Fetched 2 pages · "), "{en}");
    let (zh, en) = lines(
        &[("memory_search", r#"{"query":"猫"}"#), FETCH, SEARCH],
        done(3),
    );
    assert!(
        zh.starts_with("搜索了 1 次网络 · 1 个网页 · 1 个工具 · "),
        "排在工具前面：{zh}"
    );
    assert!(
        en.starts_with("Searched the web 1 time · 1 page · 1 tool · "),
        "{en}"
    );
    let (_, en) = lines(
        &[
            ("remember", r#"{"class":"user","text":"养猫"}"#),
            SEARCH,
            SEARCH,
        ],
        done(3),
    );
    assert!(
        en.starts_with("Remembered 1 memory · 2 searches · "),
        "排在记住后面，不打头的写 N searches：{en}"
    );
}

#[test]
fn a_failed_search_or_fetch_is_just_a_tool() {
    let (_, en) = lines(
        &[SEARCH, FETCH],
        vec![
            Play::Fails("cooling".to_string()),
            Play::Fails("404".to_string()),
        ],
    );
    assert!(en.starts_with("Used 2 tools · "), "{en}");
}

/// 结果的 `human` 带 `via` 的，标题多一格 `via`；没带的没有。
#[test]
fn the_source_goes_to_the_title() {
    let mut stage = stage();
    stage.model([Line::calls("", &[SEARCH, FETCH]), Line::says("好。")]);
    stage.tools(done(2));
    stage.say("hi");
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    for event in stage.log() {
        let mut event = event.clone();
        if let Body::ToolResult(result) = &mut event.body
            && result.human.is_none()
            && projector.entries().len() < 100
        {
            let mut said = Said::new("core/tool-results/cancelled-before");
            said.fields.insert("via".to_string(), "Exa".to_string());
            result.human = Some(said);
        }
        projector.event(&event);
    }
    let entries = projector.entries().to_vec();
    let tools: Vec<serde_json::Value> = of_kind(&entries, "tool").into_iter().map(json).collect();
    assert_eq!(tools[0]["title"]["via"], "Exa", "{}", tools[0]);
    assert_eq!(
        tools[0]["title"]["said"], "Cancelled",
        "结果那一句照旧：{}",
        tools[0]
    );
}

/// 没带 `via` 的标题没有这一格。
#[test]
fn without_a_source_there_is_no_via() {
    let mut stage = stage();
    stage.model([Line::calls("", &[FETCH]), Line::says("好。")]);
    stage.tools(done(1));
    stage.say("hi");
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    for event in stage.log() {
        projector.event(event);
    }
    let entries = projector.entries().to_vec();
    let tool = json(of_kind(&entries, "tool")[0]);
    assert!(tool["title"].get("via").is_none(), "{tool}");
}

/// 挪到最前的推送（施工 V-D 修，终端报的）：`after` 是 `null`，读回来是「挪到最前」，不是「没挪」；没挪的不带 `after`。
#[test]
fn an_update_moved_to_the_front_reads_back_as_moved() {
    let entry = r#"{"id":"m2","kind":"user","text":"hi","at":"2026-09-25T07:00:01.000Z"}"#;
    let moved = miyu_view::Change::read(
        "view.update",
        serde_json::from_str(&format!(
            r#"{{"session":"s","entry":{entry},"after":null}}"#
        ))
        .expect("JSON"),
    )
    .expect("读得懂");
    assert!(
        matches!(
            moved,
            miyu_view::Change::Update {
                after: Some(None),
                ..
            }
        ),
        "{moved:?}"
    );
    let stayed = miyu_view::Change::read(
        "view.update",
        serde_json::from_str(&format!(r#"{{"session":"s","entry":{entry}}}"#)).expect("JSON"),
    )
    .expect("读得懂");
    assert!(
        matches!(stayed, miyu_view::Change::Update { after: None, .. }),
        "{stayed:?}"
    );
    on_the_wire(&moved);
}
