//! 读出厂的工具说明（施工 M-4）：写了 `"result"` 的读进规格，没写的是 `None`。

use super::*;

/// 出厂的资源目录。
fn resources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

#[test]
fn the_result_shape_in_a_descriptor_reaches_the_spec() {
    let search = spec(&resources(), "net", "web_search", Access::Network).expect("读得出");
    let result = search.result.expect("web_search 写了结果的样子");
    assert!(result.starts_with('{'), "{result}");
    let fetch = spec(&resources(), "net", "web_fetch", Access::Network).expect("读得出");
    assert_eq!(fetch.result, None, "没写的不带");
}

/// 退役的 `need_load`（施工 L-1 登记、FL-1 拆掉）：包自带的说明文件写了的照旧读得进，规格里没有它。
#[test]
fn a_descriptor_with_a_retired_need_load_is_still_read() {
    let dir = std::env::temp_dir().join(format!("miyu-load-retired-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("tools")).expect("建得了");
    std::fs::write(
        dir.join("tools").join("lookup.json"),
        r#"{"description":"Look a word up.","parameters":{"type":"object"},"need_load":true}"#,
    )
    .expect("写得进");
    let read = spec_in(&dir, "lookup", Access::Read);
    std::fs::remove_dir_all(&dir).expect("删得了");
    let spec = read.expect("照旧读得进");
    assert_eq!(spec.description, "Look a word up.");
}
