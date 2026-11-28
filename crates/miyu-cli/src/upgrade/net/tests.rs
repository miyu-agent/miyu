//! 没拿到的怎么分：连不上、状态码。

use super::*;

fn failed(status: Option<u16>, headers: &[(&str, &str)]) -> miyu_http::Failed {
    miyu_http::Failed {
        message: "connection refused".to_string(),
        status,
        headers: headers
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect(),
        body: Vec::new(),
    }
}

/// 施工 UP-4：不走 API 了，带着 `x-ratelimit-*` 头的 403、429 也照状态码说。
#[test]
fn no_status_is_offline_and_the_rest_keep_their_status() {
    assert_eq!(
        classify(&failed(None, &[])),
        NetError::Offline("connection refused".to_string())
    );
    let limit = [
        ("x-ratelimit-remaining", "0"),
        ("x-ratelimit-reset", "1790000000"),
    ];
    for status in [403, 404, 429, 500] {
        assert_eq!(
            classify(&failed(Some(status), &limit)),
            NetError::Status(status)
        );
    }
}
