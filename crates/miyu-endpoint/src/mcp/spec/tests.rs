use miyu_config::package::McpServer;
use miyu_kernel::tool::Access;
use serde_json::json;

use super::*;

fn listed(annotations: serde_json::Value) -> miyu_mcp::Tool {
    miyu_mcp::Tool::read(&json!({
        "name": "fetch",
        "inputSchema": {"type": "object"},
        "annotations": annotations,
    }))
    .unwrap()
}

fn server(venues: &[&str]) -> McpServer {
    McpServer {
        name: "web".to_string(),
        command: vec!["web-server".to_string()],
        env: Vec::new(),
        venues: venues.iter().map(|venue| (*venue).to_string()).collect(),
        timeout_ms: 600_000,
        line: None,
    }
}

/// 名字：`mcp__<服务名>__<工具名>`，不合写法的字一个换一个 `_`，超过 64 个字符的截到 64 个。
#[test]
fn a_tool_name_is_prefixed_cleaned_and_cut() {
    assert_eq!(name("web", "fetch"), "mcp__web__fetch");
    assert_eq!(name("web", "get page.v2"), "mcp__web__get_page_v2");
    assert_eq!(name("web", "搜索-it"), "mcp__web____-it");
    let long = name("web", &"a".repeat(80));
    assert_eq!(long.len(), 64);
    assert!(long.starts_with("mcp__web__aaa"));
}

/// 说明照原文，超过 2048 个字符的截掉后面（照字符数，不截坏一个字）；没给的是空的。
#[test]
fn a_description_is_kept_up_to_2048_characters() {
    assert_eq!(description(Some("Fetch a page.")), "Fetch a page.");
    assert_eq!(description(None), "");
    let long = "页".repeat(2050);
    assert_eq!(description(Some(&long)).chars().count(), 2048);
    let exact = "a".repeat(2048);
    assert_eq!(description(Some(&exact)), exact);
}

/// 访问类别：说只读的算 `read`，说不是只读的、没说的都算 `network`。
#[test]
fn only_a_read_only_hint_makes_a_tool_read() {
    assert_eq!(access(&listed(json!({"readOnlyHint": true}))), Access::Read);
    assert_eq!(
        access(&listed(json!({"readOnlyHint": false}))),
        Access::Network
    );
    assert_eq!(access(&listed(json!({}))), Access::Network);
    assert_eq!(
        access(&listed(json!({"readOnlyHint": "yes"}))),
        Access::Network,
        "不是布尔的不算"
    );
}

/// 给哪种会话照清单：没写的（读清单时补成只给本机）只有本机；写了的照写的。
#[test]
fn venues_follow_the_manifest() {
    assert_eq!(
        venues(&server(&["local"])),
        Venues {
            local: true,
            private: false,
            group: false
        }
    );
    assert_eq!(
        venues(&server(&["group", "private"])),
        Venues {
            local: false,
            private: true,
            group: true
        }
    );
}
