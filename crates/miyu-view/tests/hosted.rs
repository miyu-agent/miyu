//! 供应商侧工具（施工 HT-3，`docs/blueprint/view.md`「种类」的 `tool`、「标题那一句」，设计 `71-供应商侧工具.md` 第三节第 4
//! 条）：模型在供应商那边自己跑的搜索、读网页画成一步工具，带 `hosted`，标题的 `via` 是写这条回复的供应商；流式时就画，
//! 和翻页一字不差；结果到了才定成了还是出错，没结果的照回复是不是打断的定；不进等结果的调用那张表。

use std::sync::Arc;

use miyu_kernel::accumulate::{Delta, Kind};
use miyu_kernel::event::Body as EventBody;
use miyu_kernel::session::Queued;
use miyu_kernel::testkit::{Line, Play, Stage, Streamed};
use miyu_view::{Entry, Projector, Texts};
use serde_json::Value;

use crate::support::*;

pub(crate) fn start(index: usize, name: &str) -> Delta {
    Delta::Start {
        index,
        kind: Kind::HostedTool {
            name: name.to_string(),
        },
    }
}

pub(crate) fn text_start(index: usize) -> Delta {
    Delta::Start {
        index,
        kind: Kind::Text,
    }
}

pub(crate) fn text(index: usize, text: &str) -> Delta {
    Delta::Text {
        index,
        text: text.to_string(),
    }
}

pub(crate) fn outcome(index: usize, text: &str, failed: bool) -> Delta {
    Delta::Outcome {
        index,
        text: text.to_string(),
        failed,
    }
}

pub(crate) fn end(index: usize) -> Delta {
    Delta::End { index }
}

/// 一件搜索 `rust`（参数分两段来），结果是 `found`，再说一句。
pub(crate) fn searched(found: Option<(&str, bool)>) -> Vec<Delta> {
    let mut deltas = vec![
        start(0, "web_search"),
        text(0, r#"{"query":"#),
        text(0, r#""rust"}"#),
    ];
    if let Some((found, failed)) = found {
        deltas.push(outcome(0, found, failed));
    }
    deltas.extend([end(0), text_start(1), text(1, "好。"), end(1)]);
    deltas
}

/// 照剧本跑一轮（模型先照 `deltas` 回），交回翻页和视图流对过的条目。
pub(crate) fn run(deltas: Vec<Delta>) -> (Stage, Vec<Entry>) {
    let mut stage = stage();
    stage.model([Line::streams(deltas)]);
    stage.say("搜一下");
    let entries = same(&stage);
    (stage, entries)
}

fn tools(entries: &[Entry]) -> Vec<Value> {
    of_kind(entries, "tool").into_iter().map(json).collect()
}

/// 有 `providers` 这几个显示名的英文字。
fn with_providers(providers: &[(&str, &str)]) -> Texts {
    let mut texts = texts("en");
    texts.providers = providers
        .iter()
        .map(|(id, name)| (id.to_string(), name.to_string()))
        .collect();
    texts
}

/// 视图流每喂一条，编号是 `id` 的那一步工具的状态和 `output`，变了才记（还没有那一步的不记）。
fn states_of(items: &[Streamed<'_>], id: &str) -> Vec<(String, Value)> {
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    let mut seen = Vec::new();
    for item in items {
        match item {
            Streamed::Event(event) => projector.event(event),
            Streamed::Transient(transient) => projector.transient(transient),
        };
        if let Some(entry) = projector.entries().iter().find(|e| e.id.as_str() == id) {
            let tool = json(entry);
            let now = (
                tool["state"].as_str().unwrap_or_default().to_string(),
                tool["output"].clone(),
            );
            if seen.last() != Some(&now) {
                seen.push(now);
            }
        }
    }
    seen
}

/// 那一步的编号：唯一的那一步工具。
fn the_step(entries: &[Entry]) -> String {
    of_kind(entries, "tool")[0].id.as_str().to_string()
}

#[test]
fn a_hosted_search_is_a_tool_step_streamed_and_paged_alike() {
    let (stage, entries) = run(searched(Some((
        "1. The Rust Language - https://rust-lang.org/",
        false,
    ))));
    let tool = &tools(&entries)[0];
    assert_eq!(tool["name"], "web_search");
    assert_eq!(tool["hosted"], true, "{tool}");
    assert_eq!(
        tool["title"]["name"], "Web search",
        "核心那件的显示名：{tool}"
    );
    assert_eq!(tool["title"]["object"], "rust", "对象照 query：{tool}");
    assert_eq!(
        tool["title"]["via"], "deepseek",
        "没有显示名的写编号：{tool}"
    );
    assert!(
        tool["title"].get("said").is_none(),
        "没有结果那一句：{tool}"
    );
    assert_eq!(tool["state"], "ok");
    assert_eq!(tool["args"], r#"{"query":"rust"}"#);
    assert_eq!(
        tool["output"],
        "1. The Rust Language - https://rust-lang.org/"
    );
    assert!(tool.get("output_cut").is_none());
    assert!(tool.get("approval").is_none());
    assert!(
        tool["call"]
            .as_str()
            .is_some_and(|c| c.starts_with("call_")),
        "落盘以后有编号：{tool}"
    );
    assert!(tool.get("took_ms").is_some(), "照块的起止：{tool}");
    let group = json(of_kind(&entries, "group")[0]);
    assert_eq!(group["steps"].as_array().map(Vec::len), Some(1));
    // 流式时：准备中、参数收全在跑、结果到了成了。
    let id = the_step(&entries);
    let states = states_of(&stage.stream(), &id);
    let names: Vec<&str> = states.iter().map(|(s, _)| s.as_str()).collect();
    assert_eq!(names, ["preparing", "ok"], "结果在 end 以前到：{states:?}");
}

#[test]
fn the_outcome_may_come_after_the_next_block_starts() {
    let deltas = vec![
        start(0, "web_search"),
        text(0, r#"{"query":"rust"}"#),
        text_start(1),
        text(1, "好。"),
        outcome(0, "found", false),
        end(0),
        end(1),
    ];
    let (stage, entries) = run(deltas);
    let id = the_step(&entries);
    let states = states_of(&stage.stream(), &id);
    assert_eq!(
        states,
        [
            ("preparing".to_string(), Value::Null),
            ("running".to_string(), Value::Null),
            ("ok".to_string(), Value::from("found")),
        ],
        "下一块开始只收参数，结果到了才定"
    );
    assert_eq!(tools(&entries)[0]["output"], "found");
}

#[test]
fn a_failed_hosted_tool_is_an_error() {
    let (_, entries) = run(searched(Some(("rate limited", true))));
    let tool = &tools(&entries)[0];
    assert_eq!(tool["state"], "error", "{tool}");
    assert_eq!(tool["output"], "rate limited");
    let group = json(of_kind(&entries, "group")[0]);
    assert!(
        group["summary_en"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|part| part["tone"] == "error"),
        "出错的算工具、标红：{group}"
    );
}

#[test]
fn without_an_outcome_a_finished_reply_means_ok() {
    let (_, entries) = run(searched(None));
    let tool = &tools(&entries)[0];
    assert_eq!(tool["state"], "ok", "供应商没报结果：{tool}");
    assert!(tool.get("output").is_none(), "{tool}");
}

#[test]
fn without_an_outcome_an_interrupted_reply_means_cancelled() {
    let mut stage = stage();
    stage.model([
        Line::streams(vec![start(0, "web_search"), text(0, r#"{"query":"rust"}"#)]).held(),
    ]);
    stage.say("搜一下");
    stage.interrupt(Queued::Send);
    let entries = same(&stage);
    let tool = &tools(&entries)[0];
    assert_eq!(tool["state"], "cancelled", "{tool}");
    assert_eq!(tool["title"]["object"], "rust", "参数照落了盘的：{tool}");
}

#[test]
fn known_names_borrow_the_core_tool_and_unknown_ones_stay_raw() {
    let (_, entries) = run(vec![
        start(0, "read_url_content"),
        text(0, r#"{"Url":"https://example.invalid/a"}"#),
        outcome(0, "page", false),
        end(0),
        start(1, "code_execution"),
        text(1, r#"{"query":"x","code":"1+1"}"#),
        outcome(1, "2", false),
        end(1),
        text_start(2),
        text(2, "好。"),
        end(2),
    ]);
    let tools = tools(&entries);
    assert_eq!(tools[0]["title"]["name"], "Fetch page", "{}", tools[0]);
    assert_eq!(
        tools[0]["title"]["object"], "https://example.invalid/a",
        "对象照表里的 Url"
    );
    assert_eq!(
        tools[1]["title"]["name"], "code_execution",
        "对不上的写原名：{}",
        tools[1]
    );
    assert!(
        tools[1]["title"].get("object").is_none(),
        "对不上的没有对象：{}",
        tools[1]
    );
    let group = json(of_kind(&entries, "group")[0]);
    let en: String = group["summary_en"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|part| part["text"].as_str())
        .collect();
    assert!(
        en.starts_with("Fetched 1 page · 1 tool · "),
        "读网页单算，对不上的算别的工具：{en}"
    );
}

#[test]
fn a_hosted_search_counts_as_a_web_search() {
    let (_, entries) = run(vec![
        start(0, "WebSearch"),
        text(0, r#"{"query":"a"}"#),
        outcome(0, "x", false),
        end(0),
        start(1, "search_web"),
        text(1, r#"{"query":"b"}"#),
        outcome(1, "y", false),
        end(1),
        text_start(2),
        text(2, "好。"),
        end(2),
    ]);
    let group = json(of_kind(&entries, "group")[0]);
    let zh_free: String = group["summary_en"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|part| part["text"].as_str())
        .collect();
    assert!(
        zh_free.starts_with("Searched the web 2 times · "),
        "{zh_free}"
    );
}

#[test]
fn via_is_the_display_name_of_the_provider() {
    let (stage, _) = run(searched(Some(("found", false))));
    let mut paged = Projector::new(Arc::new(with_providers(&[("deepseek", "DeepSeek")])), None);
    for event in stage.log() {
        paged.event(event);
    }
    let mut live = Projector::new(Arc::new(with_providers(&[("deepseek", "DeepSeek")])), None);
    for item in stage.stream() {
        match item {
            Streamed::Event(event) => live.event(event),
            Streamed::Transient(transient) => live.transient(transient),
        };
    }
    assert_eq!(paged.entries(), live.entries());
    let tool = &tools(paged.entries())[0];
    assert_eq!(tool["title"]["via"], "DeepSeek", "{tool}");
    let mut other = Projector::new(
        Arc::new(with_providers(&[("anthropic", "Anthropic")])),
        None,
    );
    for event in stage.log() {
        other.event(event);
    }
    assert_eq!(
        tools(other.entries())[0]["title"]["via"],
        "deepseek",
        "别家的显示名不算"
    );
}

#[test]
fn a_long_outcome_is_cut_like_a_tool_result() {
    let long: String = (1..=150).map(|n| format!("result {n}\n")).collect();
    let (_, entries) = run(searched(Some((&long, false))));
    let tool = &tools(&entries)[0];
    let output = tool["output"].as_str().expect("有 output");
    assert_eq!(output.lines().count(), 100, "{output}");
    assert_eq!(tool["output_cut"], true);
    let wide = "字".repeat(4000);
    let (_, entries) = run(searched(Some((&wide, false))));
    let tool = &tools(&entries)[0];
    let output = tool["output"].as_str().expect("有 output");
    assert!(
        output.len() <= 8 * 1024 && wide.starts_with(output),
        "切在字上"
    );
    assert_eq!(tool["output_cut"], true);
}

#[test]
fn mixed_with_a_core_tool_each_keeps_its_own_result() {
    let mut stage = stage();
    stage.model([
        Line::streams(vec![
            Delta::Start {
                index: 0,
                kind: Kind::ToolCall {
                    name: "read".to_string(),
                },
            },
            text(0, r#"{"file_path":"a"}"#),
            end(0),
            start(1, "web_search"),
            text(1, r#"{"query":"rust"}"#),
            outcome(1, "searched", false),
            end(1),
        ]),
        Line::says("好。"),
    ]);
    stage.tools([Play::done("file a")]);
    stage.say("读一下再搜");
    let entries = same(&stage);
    let tools = tools(&entries);
    assert_eq!(tools.len(), 2, "{tools:#?}");
    assert_eq!(tools[0]["name"], "read");
    assert!(
        tools[0].get("hosted").is_none(),
        "核心执行的不写：{}",
        tools[0]
    );
    assert_eq!(tools[0]["output"], "file a", "核心工具的结果对上自己那一步");
    assert_eq!(tools[1]["name"], "web_search");
    assert_eq!(tools[1]["output"], "searched", "供应商侧工具不被抢");
    let (read_call, hosted_call) = (tools[0]["call"].clone(), tools[1]["call"].clone());
    assert_ne!(read_call, hosted_call);
    // 工具结果的编号换成供应商侧工具的：它不在等结果的表里，结果谁也找不到。
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    for event in stage.log() {
        let mut event = event.clone();
        if let EventBody::ToolResult(result) = &mut event.body {
            result.call_id = serde_json::from_value(hosted_call.clone()).expect("编号");
        }
        projector.event(&event);
    }
    let forged = self::tools(projector.entries());
    assert_eq!(forged[1]["output"], "searched", "{}", forged[1]);
    assert_eq!(forged[1]["state"], "ok");
    assert_eq!(
        forged[0]["state"], "cancelled",
        "read 的结果没到：{}",
        forged[0]
    );
}
