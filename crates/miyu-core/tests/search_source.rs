//! 「联网服务」的「DeepSeek Responses 接口内置」这个选项（设计 72 第二节第 1、2 条，施工 DSR-2）：在 `config.schema` 里，带名字、
//! 说明和选中时弹的那一句提示（`notice`）；别的选项没有提示；能写进配置。

use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, json};

use crate::support::{Head, Home};
use miyu_core::serve;
use miyu_session::testkit::Script;

/// 这一项的键和这个选项。
const KEY: &str = "net.search.source";
const OPTION: &str = "deepseek-responses";

/// 三种语言的字表里这一项的 `field`（`options`、`option_descriptions`、`option_notices`）给这个选项写的那一句。
fn written(field: &str) -> Vec<String> {
    ["zh", "en", "ja"]
        .iter()
        .map(|language| {
            let file = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../resources/core/human/{language}.json"));
            let text = std::fs::read_to_string(&file).expect("读得到");
            let json: Value = serde_json::from_str(&text).expect("是 JSON");
            json["config"]["items"][KEY][field][OPTION]
                .as_str()
                .unwrap_or_else(|| panic!("{language} 的 {field} 写了这个选项"))
                .to_string()
        })
        .collect()
}

#[tokio::test]
async fn the_schema_lists_the_option_with_its_notice_and_a_set_takes_it() {
    let home = Home::new();
    let opened = home.open();
    let core = home.core_configured(Arc::new(Script::new([])), &opened.token);
    let running = tokio::spawn(serve(
        opened.listener,
        core,
        || None,
        std::future::pending(),
    ));
    let mut head = Head::connect(&home.root).await;
    let reply = head
        .call(
            "source-72d1a09c4e5b3f68-1",
            "config.schema",
            json!({"keys": [KEY]}),
        )
        .await;
    let items = reply["result"]["items"].as_array().expect("有项");
    assert_eq!(items.len(), 1, "{reply}");
    let options = items[0]["options"].as_array().expect("有选项");
    let values: Vec<&str> = options
        .iter()
        .filter_map(|option| option["value"].as_str())
        .collect();
    let at = values
        .iter()
        .position(|value| *value == OPTION)
        .unwrap_or_else(|| panic!("有这个选项：{values:?}"));
    assert_eq!(values[at - 1], "deepseek", "接在 DeepSeek 搜索接口后面");
    let option = &options[at];
    for (field, written) in [
        ("name", written("options")),
        ("description", written("option_descriptions")),
        ("notice", written("option_notices")),
    ] {
        let said = option[field].as_str().unwrap_or_default();
        assert!(written.iter().any(|text| text == said), "{field}：{option}");
    }
    // 别的选项没有选中时弹的提示。
    for other in options.iter().filter(|other| other["value"] != OPTION) {
        assert!(other.get("notice").is_none(), "{other}");
    }
    let set = head
        .call(
            "source-72d1a09c4e5b3f68-2",
            "config.set",
            json!({"layer": "system", "changes": [{"key": KEY, "value": OPTION}]}),
        )
        .await;
    assert!(set.get("error").is_none(), "{set}");
    running.abort();
}
