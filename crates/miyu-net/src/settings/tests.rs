//! 搜索来源的选项和出厂的来源对得上（施工 N-2 下）：下拉里能选的、`search.json` 里有的，一一对应。

use std::path::Path;

use super::{AUTO, SearchSettings};

#[test]
fn the_options_are_auto_and_every_shipped_source() {
    let item = &SearchSettings::ITEMS[0];
    assert_eq!(item.key, "net.search.source");
    let miyu_config::Kind::Option(options) = item.kind else {
        panic!("是选项：{:?}", item.kind);
    };
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/software/net/search.json"),
    )
    .expect("读得了");
    let shipped: serde_json::Value = serde_json::from_str(&text).expect("是 JSON");
    let mut sources: Vec<&str> = shipped["sources"]
        .as_object()
        .expect("有 sources")
        .keys()
        .map(String::as_str)
        .collect();
    sources.sort_unstable();
    let mut listed: Vec<&str> = options
        .iter()
        .copied()
        .filter(|option| *option != AUTO)
        .collect();
    listed.sort_unstable();
    assert_eq!(listed, sources, "下拉里的来源就是出厂的那几家");
    assert_eq!(options[0], AUTO, "自动排第一，是默认值");
}
