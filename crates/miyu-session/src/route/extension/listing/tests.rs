//! `driver.models` 的回答怎么读（施工 MD-4）：名字、窗口、最大输出、工具、看图照收，思考认得、照收不用；多了格、不是对象、少了名字的整份不要。

use serde_json::json;

use super::*;

#[test]
fn a_model_list_reads_what_the_facts_use() {
    let read = models(json!({"models": [
        {"id": "ag-pro", "window": 1_000_000, "max_output": 65_536, "tools": true, "vision": true, "reasoning": true},
        {"id": "ag-lite"}
    ]}))
    .expect("读得出");
    assert_eq!(
        read,
        [
            ListedModel {
                id: "ag-pro".to_string(),
                window: Some(1_000_000),
                max_output: Some(65_536),
                tools: Some(true),
                vision: Some(true),
            },
            ListedModel {
                id: "ag-lite".to_string(),
                ..ListedModel::default()
            },
        ]
    );
    assert_eq!(models(json!({"models": []})), Ok(Vec::new()));
}

#[test]
fn a_bad_model_list_is_refused_whole() {
    for bad in [
        json!([]),
        // serde 会把数组读成「只写了 models」的结构：回答一定是对象。
        json!([[]]),
        json!({}),
        json!({"models": [{"window": 8}]}),
        json!({"models": [{"id": "x", "price": 1}]}),
        json!({"models": [{"id": "x", "window": "big"}]}),
        json!({"models": [], "next": 2}),
    ] {
        assert!(models(bad.clone()).is_err(), "{bad}");
    }
}
