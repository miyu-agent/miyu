use serde_json::json;

use super::{Param, headers, of};

fn param(name: &str, path: &[&str]) -> Param {
    Param {
        name: name.to_string(),
        path: path.iter().map(|key| (*key).to_string()).collect(),
    }
}

#[test]
fn marks_reached_through_properties_alone_are_kept() {
    let schema = json!({"type": "object", "properties": {
        "region": {"type": "string", "x-mcp-header": "Region"},
        "query": {"type": "string"},
        "where": {"type": "object", "properties": {"zone": {"type": "integer", "x-mcp-header": "Zone"}}},
        "x-mcp-header": {"type": "string", "default": {"x-mcp-header": "not a mark"}},
    }});
    let mut found = of(&schema).expect("合规矩");
    found.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(
        found,
        [
            param("Region", &["region"]),
            param("Zone", &["where", "zone"])
        ]
    );
    assert_eq!(of(&json!({"type": "object"})), Ok(Vec::new()));
}

#[test]
fn a_mark_breaking_the_rules_rejects_the_tool() {
    let broken = [
        json!({"type": "object", "properties": {"a": {"type": "string", "x-mcp-header": ""}}}),
        json!({"type": "object", "properties": {"a": {"type": "string", "x-mcp-header": "A B"}}}),
        json!({"type": "object", "properties": {"a": {"type": "string", "x-mcp-header": "A:B"}}}),
        json!({"type": "object", "properties": {"a": {"type": "string", "x-mcp-header": "A\r\nX: 1"}}}),
        json!({"type": "object", "properties": {"a": {"type": "string", "x-mcp-header": 1}}}),
        json!({"type": "object", "properties": {"a": {"type": "number", "x-mcp-header": "A"}}}),
        json!({"type": "object", "properties": {"a": {"x-mcp-header": "A"}}}),
        json!({"type": "object", "properties": {"a": {"type": "array", "items": {"type": "string", "x-mcp-header": "A"}}}}),
        json!({"type": "object", "properties": {"a": {"anyOf": [{"type": "string", "x-mcp-header": "A"}]}}}),
        json!({"type": "object", "$defs": {"a": {"type": "string", "x-mcp-header": "A"}}}),
        json!({"type": "string", "x-mcp-header": "Root"}),
        json!({"type": "object", "properties": {
            "a": {"type": "string", "x-mcp-header": "Same"},
            "b": {"type": "string", "x-mcp-header": "same"},
        }}),
    ];
    for schema in broken {
        assert!(of(&schema).is_err(), "{schema}");
    }
}

#[test]
fn values_are_read_at_the_path_and_written_as_text() {
    let params = [
        param("Region", &["region"]),
        param("Zone", &["where", "zone"]),
        param("Dry", &["dry"]),
        param("Missing", &["missing"]),
        param("Empty", &["empty"]),
        param("Wide", &["wide"]),
    ];
    let arguments = json!({"region": "华东", "where": {"zone": 42.0}, "dry": false, "empty": null, "wide": 1.5});
    assert_eq!(
        headers(&params, &arguments),
        [
            (
                "Mcp-Param-Region".to_string(),
                super::super::header_value("华东")
            ),
            ("Mcp-Param-Zone".to_string(), "42".to_string()),
            ("Mcp-Param-Dry".to_string(), "false".to_string()),
        ],
        "没有值、null、带小数的不带"
    );
}
