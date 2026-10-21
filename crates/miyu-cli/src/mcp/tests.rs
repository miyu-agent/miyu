//! `miyu mcp login`（施工 X-42 上）：照 `mcp.list` 认服务的状态；打开网址的程序照系统挑，网址原样一个参数交过去。

use serde_json::json;

use super::*;

#[test]
fn the_state_comes_from_the_listed_server() {
    let listed = json!({"servers": [
        {"name": "other", "state": "running"},
        {"name": "notes", "state": "stopped", "reason": "needs_login"},
    ]});
    assert_eq!(state(&listed, "notes"), Some("stopped"));
    assert_eq!(state(&listed, "other"), Some("running"));
    assert_eq!(state(&listed, "carried"), None, "包里带的不在 mcp.list 里");
}

#[test]
fn the_url_goes_whole_to_the_opener() {
    let url = "https://auth.example.com/authorize?a=1&b=2";
    let (program, args) = opener(url);
    assert!(!program.is_empty());
    assert_eq!(
        args.last().map(String::as_str),
        Some(url),
        "网址带着 & 也是一个参数"
    );
}
