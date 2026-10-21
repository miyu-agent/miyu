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
