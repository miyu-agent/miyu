//! `session.context` 的样子（施工 CX-1 上）：块照编号、名字照语言（包、服务照原名），只露名字的工具归待加载，记忆的常驻、
//! 召回和知识库想起的单列，是 0 的不交，有锚的是 measured。

use serde_json::json;

use miyu_kernel::id::{FactKind, ModuleId};
use miyu_kernel::session::{Breakdown, ContextLimits, Said};
use miyu_session::{Context, SystemPart};

use super::{Namer, shape};

fn fact(module: &str, kind: &str) -> Said {
    Said::Fact {
        module: Some(ModuleId::parse(module).unwrap()),
        kind: FactKind::parse(kind).unwrap(),
    }
}

fn context(anchor: Option<u64>) -> Context {
    Context {
        breakdown: Breakdown {
            limits: ContextLimits {
                window: Some(128_000),
                compaction_line: Some(102_400),
            },
            used: Some(1118),
            anchor,
            system: 300,
            tools: vec![
                ("glob".to_string(), 20),
                ("mcp__web__fetch".to_string(), 40),
                ("read".to_string(), 50),
                ("shell".to_string(), 60),
                ("weather".to_string(), 30),
                ("web_search".to_string(), 5),
            ],
            demos: 0,
            summary: 70,
            said: vec![
                (fact("memory", "memory"), 15),
                (Said::User, 100),
                (fact("knowledge", "recall"), 25),
                (fact("memory", "recall"), 35),
                (fact("env", "env"), 8),
                (fact("project-docs", "project-docs"), 40),
                (Said::Assistant, 120),
                (Said::ToolResult, 200),
            ],
        },
        system: vec![
            (SystemPart::Skills, 40),
            (SystemPart::Appended("weather-pack".to_string()), 10),
            (SystemPart::Other, 250),
        ],
        offered: ["mcp__web__fetch", "read", "shell", "weather", "web_search"]
            .into_iter()
            .map(str::to_string)
            .collect(),
    }
}

fn namer_shape(context: &Context) -> serde_json::Value {
    let word = |part: &str| format!("<{part}>");
    let tool = |name: &str| match name {
        "mcp__web__fetch" => ("tools.mcp:web".to_string(), Some("web".to_string())),
        "weather" => (
            "tools.package:weather-pack".to_string(),
            Some("天气".to_string()),
        ),
        _ => ("tools.builtin".to_string(), None),
    };
    let appended = |package: &str| format!("包 {package}");
    shape(
        context,
        &Namer {
            word: &word,
            tool: &tool,
            appended: &appended,
        },
    )
}

#[test]
fn the_context_comes_in_named_parts() {
    // 用量正好是各块估的和：摊了也是原值。
    let leaf = |part: &str, tokens: u64| json!({"part": part, "label": format!("<{part}>"), "tokens": tokens});
    assert_eq!(
        namer_shape(&context(Some(800))),
        json!({
            "window": 128_000,
            "line": 102_400,
            "used": 1118,
            "anchor": 800,
            "basis": "measured",
            "estimated_total": 300 + 205 + 70 + 543,
            "parts": [
                {"part": "system", "label": "<system>", "tokens": 300, "children": [
                    leaf("system.skills", 40),
                    {"part": "system.appended:weather-pack", "label": "包 weather-pack", "tokens": 10},
                    leaf("system.other", 250),
                ]},
                {"part": "tools", "label": "<tools>", "tokens": 205, "children": [
                    leaf("tools.builtin", 135),
                    {"part": "tools.mcp:web", "label": "web", "tokens": 40},
                    {"part": "tools.package:weather-pack", "label": "天气", "tokens": 30},
                ]},
                leaf("summary", 70),
                {"part": "messages", "label": "<messages>", "tokens": 543, "children": [
                    leaf("messages.user", 100),
                    {"part": "messages.memory", "label": "<messages.memory>", "tokens": 50, "children": [
                        leaf("messages.memory.resident", 15),
                        leaf("messages.memory.recall", 35),
                    ]},
                    leaf("messages.knowledge", 25),
                    leaf("messages.facts", 8),
                    leaf("messages.project_docs", 40),
                    leaf("messages.assistant", 120),
                    leaf("messages.tool_results", 200),
                ]},
            ],
        })
    );
}

#[test]
fn without_an_anchor_it_is_estimated_and_empty_parts_are_left_out() {
    let mut empty = context(None);
    empty.breakdown.limits = ContextLimits {
        window: None,
        compaction_line: None,
    };
    empty.breakdown.used = None;
    empty.breakdown.summary = 0;
    empty.breakdown.said.retain(|(said, _)| *said == Said::User);
    let shaped = namer_shape(&empty);
    assert_eq!(shaped["basis"], "estimated");
    for absent in ["window", "line", "used", "anchor"] {
        assert!(shaped.get(absent).is_none(), "{absent}: {shaped}");
    }
    let parts: Vec<&str> = shaped["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|part| part["part"].as_str().unwrap())
        .collect();
    assert_eq!(
        parts,
        ["system", "tools", "messages"],
        "示范、摘要是 0 不交"
    );
    assert_eq!(
        shaped["parts"][2]["children"],
        json!([{"part": "messages.user", "label": "<messages.user>", "tokens": 100}]),
        "没有记忆的不交 messages.memory"
    );
}

/// 核心照 `used` 摊（施工 CX-1 下，主会话定）：每一块的叶子照比例摊、余数给小数部分大的，加起来正好是 `used`，上一层是下面的和；
/// `estimated_total` 照旧是摊之前的；没有 `used` 的照原值。
#[test]
fn the_parts_are_scaled_to_add_up_to_used() {
    let mut halved = context(Some(800));
    halved.breakdown.used = Some(559);
    let shaped = namer_shape(&halved);
    assert_eq!(shaped["estimated_total"], 1118, "摊之前的和");
    let parts = shaped["parts"].as_array().unwrap();
    let sum: u64 = parts
        .iter()
        .map(|part| part["tokens"].as_u64().unwrap())
        .sum();
    assert_eq!(sum, 559, "{shaped}");
    for part in parts {
        if let Some(children) = part["children"].as_array() {
            let below: u64 = children
                .iter()
                .map(|child| child["tokens"].as_u64().unwrap())
                .sum();
            assert_eq!(part["tokens"], below, "上一层是下面的和：{part}");
        }
    }
    let system = &parts[0];
    assert_eq!(system["tokens"], 150, "一半：{system}");
    assert_eq!(system["children"][0]["tokens"], 20);
    // 1 118 里的 5 摊成 2.5，四舍五入的余数照小数部分大的先给，不会凭空多出、少掉。
    let mut doubled = context(Some(800));
    doubled.breakdown.used = Some(2236);
    let doubled = namer_shape(&doubled);
    assert_eq!(doubled["parts"][1]["tokens"], 410, "工具翻倍：{doubled}");
    // 摊完成了 0 的块不交。
    let mut tiny = context(Some(800));
    tiny.breakdown.used = Some(10);
    let tiny = namer_shape(&tiny);
    let mut zeros = 0;
    for part in tiny["parts"].as_array().unwrap() {
        for child in part["children"].as_array().into_iter().flatten() {
            zeros += usize::from(child["tokens"] == 0);
        }
    }
    assert_eq!(zeros, 0, "{tiny}");
    let mut unknown = context(None);
    unknown.breakdown.used = None;
    assert_eq!(
        namer_shape(&unknown)["parts"][0]["tokens"],
        300,
        "算不了用量的照原值"
    );
}
