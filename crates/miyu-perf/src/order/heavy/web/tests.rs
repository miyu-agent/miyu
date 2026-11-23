//! 假网页服务认请求、定回应（施工 LW-30）。

use super::*;

#[test]
fn proxy_requests_get_pages_and_others_get_routes() {
    let pages = Mutex::new(0);
    let get = |target: &str| Request {
        method: "GET".to_string(),
        target: target.to_string(),
        authorization: String::new(),
        body: Vec::new(),
    };
    let page = route(&get("http://pages.perf.invalid/a.html"), 9, &pages);
    assert_eq!(page.status, "200 OK");
    assert!(String::from_utf8_lossy(&page.body).contains("<table>"));
    assert_eq!(*pages.lock().unwrap_or_else(PoisonError::into_inner), 1);
    assert_eq!(
        route(&get("http://example.com/"), 9, &pages).status,
        "404 Not Found"
    );
    let search = route(&get("/search?q=ordering&format=json"), 9, &pages);
    let hits: Value = serde_json::from_slice(&search.body).expect("是 JSON");
    assert_eq!(hits["results"].as_array().map(Vec::len), Some(7));
}

#[test]
fn the_login_server_asks_for_a_token_then_answers() {
    let pages = Mutex::new(0);
    let post = |authorization: &str| Request {
        method: "POST".to_string(),
        target: "/mcp2".to_string(),
        authorization: authorization.to_string(),
        body: br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#.to_vec(),
    };
    let refused = route(&post(""), 9, &pages);
    assert_eq!(refused.status, "401 Unauthorized");
    assert!(refused.extra[0].1.contains("oauth-protected-resource"));
    assert_eq!(
        route(&post("Bearer token-perf"), 9, &pages).status,
        "200 OK"
    );
    let authorize = Request {
        method: "GET".to_string(),
        target: "/authorize?redirect_uri=http%3A%2F%2F127.0.0.1%3A5%2Fcallback&state=abc"
            .to_string(),
        authorization: String::new(),
        body: Vec::new(),
    };
    let to = &route(&authorize, 9, &pages).extra[0].1;
    assert!(
        to.starts_with(
            "http://127.0.0.1:5/callback?code=code-perf&state=abc&iss=http%3A%2F%2F127.0.0.1%3A9"
        ),
        "{to}"
    );
}

#[test]
fn the_request_line_and_headers_are_read() {
    let request =
        parse("POST /mcp HTTP/1.1\r\nHost: x\r\nauthorization: Bearer t\r\nContent-Length: 3")
            .expect("读得出");
    assert_eq!(
        (
            request.method.as_str(),
            request.target.as_str(),
            request.authorization.as_str()
        ),
        ("POST", "/mcp", "Bearer t")
    );
    assert_eq!(decode("a%20b+c%2"), "a b c%2");
}
