//! 跨没跨主机、跨了去掉哪些头（施工 AU-003）。

use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};

use super::*;

fn url(text: &str) -> Url {
    Url::parse(text).expect("合网址的写法")
}

/// 修以前：同一个端口从 https 换成 http 不算跨了主机，密钥明着发出去（审计 #003 第 3 条）。
#[test]
fn changing_the_scheme_on_the_same_port_counts_as_crossing() {
    assert!(crossed(
        &url("http://a.example:8443/x"),
        &url("https://a.example:8443/v1")
    ));
    assert!(crossed(
        &url("https://b.example/x"),
        &url("https://a.example/x")
    ));
    assert!(crossed(
        &url("https://a.example:8443/x"),
        &url("https://a.example/x")
    ));
    assert!(!crossed(
        &url("https://a.example/y"),
        &url("https://a.example:443/x")
    ));
}

/// 标了敏感的、reqwest 那几样都去掉，别的照留。
#[test]
fn stripping_drops_what_is_marked_sensitive() {
    let mut headers = HeaderMap::new();
    let mut key = HeaderValue::from_static("sk-ant");
    key.set_sensitive(true);
    headers.insert("x-api-key", key);
    headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer sk"));
    headers.insert("cookie2", HeaderValue::from_static("a=b"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    strip(&mut headers);
    let left: Vec<&str> = headers.keys().map(HeaderName::as_str).collect();
    assert_eq!(left, ["content-type"]);
}
