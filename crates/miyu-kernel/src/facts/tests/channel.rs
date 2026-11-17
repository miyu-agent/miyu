//! 这一轮的话从哪来那一块（施工 VN-1，`facts/channel.rs`）：照开这一轮的那句话的 `by` 挑一句；本机的头照种类，不认识的、没报的
//! 用「别的」；到点叫醒的是没人在场；别的来源不挑；和最近一块一样的不再注入，换了地方才追加；压缩以后重新说；没有这几句的、
//! 安静的没有这一块。

use std::collections::BTreeMap;

use super::*;

/// 替身的几句：一眼认得出是哪一句。
fn channels() -> Channels {
    Channels {
        heads: BTreeMap::from([
            ("tui".to_string(), "<c>tui</c>".to_string()),
            ("web".to_string(), "<c>web</c>".to_string()),
        ]),
        other: "<c>other</c>".to_string(),
        nobody: "<c>nobody</c>".to_string(),
    }
}

fn with_channels() -> FactTemplates {
    templates().with_channels(Some(channels()))
}

fn by(json: &str) -> By {
    serde_json::from_str(json).unwrap()
}

const WEB: &str = r#"{"kind":"person","head":"web"}"#;
const TUI: &str = r#"{"kind":"person","head":"tui"}"#;
const LOOP: &str = r#"{"kind":"schedule","schedule":"t1","loop":"dynamic"}"#;

/// 这个边界上注入什么；不注入的是 `None`。
fn told(templates: &FactTemplates, log: &Log, asked: Option<&str>) -> Option<String> {
    let asked = asked.map(by);
    templates.channel(&log.history, asked.as_ref()).map(|fact| {
        assert_eq!(fact.kind.as_str(), CHANNEL);
        assert!(!fact.after);
        fact.text
    })
}

/// 开一个回合，内核在里面记下这一块 `text`。
fn saw(log: &mut Log, text: &str) {
    log.start();
    log.inject(KERNEL, CHANNEL, text);
    log.end();
}

#[test]
fn each_head_kind_has_its_own_line() {
    let log = Log::new();
    let facts = with_channels();
    assert_eq!(told(&facts, &log, Some(WEB)).as_deref(), Some("<c>web</c>"));
    assert_eq!(told(&facts, &log, Some(TUI)).as_deref(), Some("<c>tui</c>"));
    let unknown = r#"{"kind":"person","head":"tray"}"#;
    assert_eq!(
        told(&facts, &log, Some(unknown)).as_deref(),
        Some("<c>other</c>")
    );
    assert_eq!(
        told(&facts, &log, Some(ALICE)).as_deref(),
        Some("<c>other</c>"),
        "没报种类的照别的头"
    );
}

#[test]
fn a_schedule_means_nobody_is_there() {
    let log = Log::new();
    assert_eq!(
        told(&with_channels(), &log, Some(LOOP)).as_deref(),
        Some("<c>nobody</c>")
    );
}

#[test]
fn other_sources_pick_nothing() {
    let log = Log::new();
    let facts = with_channels();
    for other in [
        r#"{"kind":"person","via":"qq:10001"}"#,
        r#"{"kind":"external","venue":"qq:group:1","id":"qq:2"}"#,
        r#"{"kind":"harness","name":"codex"}"#,
        r#"{"kind":"session","id":"01a0f233-cfec-7023-8ed5-2a037a1d5ec8"}"#,
        MEMORY,
        KERNEL,
    ] {
        assert_eq!(told(&facts, &log, Some(other)), None, "{other}");
    }
    assert_eq!(told(&facts, &log, None), None, "没有开这一轮的话");
}

#[test]
fn the_same_place_is_not_said_twice_and_a_new_place_is() {
    let mut log = Log::new();
    let facts = with_channels();
    saw(&mut log, "<c>web</c>");
    assert_eq!(told(&facts, &log, Some(WEB)), None, "同一个地方不再说");
    assert_eq!(told(&facts, &log, Some(TUI)).as_deref(), Some("<c>tui</c>"));
    saw(&mut log, "<c>tui</c>");
    assert_eq!(
        told(&facts, &log, Some(WEB)).as_deref(),
        Some("<c>web</c>"),
        "比的是最近的那一块"
    );
    saw(&mut log, "<c>web</c>");
    assert_eq!(
        told(&facts, &log, Some(r#"{"kind":"harness","name":"codex"}"#)),
        None
    );
    assert_eq!(
        told(&facts, &log, Some(WEB)),
        None,
        "别的来源说的不改最近那一块"
    );
}

#[test]
fn after_compaction_it_is_said_again() {
    let mut log = Log::new();
    let facts = with_channels();
    log.start();
    log.inject(KERNEL, CHANNEL, "<c>web</c>");
    log.compact();
    log.end();
    assert_eq!(told(&facts, &log, Some(WEB)).as_deref(), Some("<c>web</c>"));
}

#[test]
fn a_module_block_of_the_same_kind_does_not_count() {
    let mut log = Log::new();
    log.start();
    log.inject(MEMORY, CHANNEL, "<c>web</c>");
    log.end();
    assert_eq!(
        told(&with_channels(), &log, Some(WEB)).as_deref(),
        Some("<c>web</c>")
    );
}

#[test]
fn without_the_lines_or_when_quiet_there_is_none() {
    let log = Log::new();
    assert_eq!(
        told(&templates(), &log, Some(WEB)),
        None,
        "场所会话、以前的快照"
    );
    assert_eq!(
        told(&templates().with_channels(None), &log, Some(WEB)),
        None
    );
    assert_eq!(told(&with_channels().quiet(), &log, Some(WEB)), None);
}
