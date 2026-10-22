//! `session.context` 的样子（施工 CX-1 上）：块照编号、名字照语言（包、服务照原名），只露名字的工具归待加载，记忆的常驻、
//! 召回和知识库想起的单列，是 0 的不交，有锚的是 measured。

use serde_json::json;

use miyu_kernel::id::{FactKind, ModuleId};
use miyu_kernel::session::{Breakdown, ContextLimits, Said};
use miyu_policy::Offered;
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
            used: Some(900),
            anchor,
            system: 300,
            tools: vec![
                ("load_tools".to_string(), 20),
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
                (Said::Assistant, 120),
                (Said::ToolResult, 200),
            ],
        },
        system: vec![
            (SystemPart::Skills, 40),
            (SystemPart::Appended("weather-pack".to_string()), 10),
            (SystemPart::Other, 250),
        ],
        offered: ["load_tools", "mcp__web__fetch", "read", "shell", "weather"]
            .into_iter()
            .map(|name| Offered {
                name: name.to_string(),
                loaded: true,
            })
            .chain([Offered {
                name: "web_search".to_string(),
                loaded: false,
            }])
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
    let leaf = |part: &str, tokens: u64| json!({"part": part, "label": format!("<{part}>"), "tokens": tokens});
    assert_eq!(
        namer_shape(&context(Some(800))),
        json!({
            "window": 128_000,
            "line": 102_400,
            "used": 900,
            "anchor": 800,
            "basis": "measured",
            "estimated_total": 300 + 205 + 70 + 503,
            "parts": [
                {"part": "system", "label": "<system>", "tokens": 300, "children": [
                    leaf("system.skills", 40),
                    {"part": "system.appended:weather-pack", "label": "包 weather-pack", "tokens": 10},
                    leaf("system.other", 250),
                ]},
                {"part": "tools", "label": "<tools>", "tokens": 205, "children": [
                    leaf("tools.builtin", 130),
                    {"part": "tools.mcp:web", "label": "web", "tokens": 40},
                    {"part": "tools.package:weather-pack", "label": "天气", "tokens": 30},
                    leaf("tools.deferred", 5),
                ]},
                leaf("summary", 70),
                {"part": "messages", "label": "<messages>", "tokens": 503, "children": [
                    leaf("messages.user", 100),
                    {"part": "messages.memory", "label": "<messages.memory>", "tokens": 50, "children": [
                        leaf("messages.memory.resident", 15),
                        leaf("messages.memory.recall", 35),
                    ]},
                    leaf("messages.knowledge", 25),
                    leaf("messages.facts", 8),
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
