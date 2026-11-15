//! 收录时写进索引条目的 `hosted`（施工 RB-1）：模型驱动会让供应商自己跑的工具，驱动名到清单 `[[driver]] hosted` 的原样，没写
//! `hosted` 的驱动不列；一个都没写的包这一格不出，以前生成的、没有这一格的索引照旧读得成。

use super::*;

/// 一个扩展包，带两个驱动：`ag` 写了 `hosted`，`lite` 没写。
const DRIVERS: &str = "protocol = [1, 1]\n\n[command]\nname = \"gravity\"\nprogram = \"gravity\"\nabout = { en = \"Gravity\" }\n\n[process]\ncapabilities = [\"model.driver\"]\n\n[[driver]]\nname = \"ag\"\nhosted = [\"search_web\", \"read_url_content\"]\n\n[[driver]]\nname = \"lite\"\n";

#[test]
fn an_entry_carries_the_tools_its_drivers_have_the_provider_run() {
    let scratch = Scratch::new();
    let mut net = Net::default();
    net.release(
        "miyu-agent/gravity",
        &[package(scratch.path(), "gravity", "1.0", DRIVERS, None)],
    );
    net.release(
        "miyu-agent/plain",
        &[package(scratch.path(), "plain", "1.0", "", None)],
    );
    let got = collect(
        &rules(&["miyu-agent/gravity", "miyu-agent/plain"]),
        &net,
        API,
        &scratch_dir(&scratch),
        "2026-11-14T03:00:00Z",
    );
    assert_eq!(got.index.packages.len(), 2, "{:?}", got.notes);
    let entry = |id: &str| {
        got.index
            .packages
            .iter()
            .find(|one| one.id == id)
            .expect("收了")
    };
    assert_eq!(
        entry("gravity").hosted,
        BTreeMap::from([(
            "ag".to_string(),
            vec!["search_web".to_string(), "read_url_content".to_string()]
        )]),
        "照清单原样；没写 hosted 的驱动不列"
    );
    assert!(entry("plain").hosted.is_empty());
    let written = serde_json::to_value(&got.index).expect("写得成");
    let plain = written["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|one| one["id"] == "plain")
        .expect("在");
    assert!(plain.get("hosted").is_none(), "一个都没写的不出这一格");
    let back = Index::parse(&serde_json::to_vec(&got.index).expect("写得成")).expect("读得回去");
    assert_eq!(back, got.index);
}
