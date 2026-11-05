//! 没拿到的怎么分：连不上、限流、别的状态码。

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

#[test]
fn no_status_is_offline_a_used_up_limit_is_limited_and_the_rest_keep_their_status() {
    assert_eq!(
        classify(&failed(None, &[])),
        NetError::Offline("connection refused".to_string())
    );
    let limit = [
        ("x-ratelimit-remaining", "0"),
        ("x-ratelimit-reset", "1790000000"),
    ];
    assert_eq!(
        classify(&failed(Some(403), &limit)),
        NetError::Limited(Some(1_790_000_000))
    );
    assert_eq!(
        classify(&failed(Some(429), &[("x-ratelimit-remaining", "0")])),
        NetError::Limited(None)
    );
    assert_eq!(
        classify(&failed(Some(403), &[("x-ratelimit-remaining", "12")])),
        NetError::Status(403),
        "没用完的 403 是别的原因"
    );
    assert_eq!(classify(&failed(Some(404), &limit)), NetError::Status(404));
}
