//! 走 REST 的几家（AnySearch、DeepSeek、Tavily、Brave、SearXNG，施工 N-2 上、N-3 上）：照 [`Rest`] 发、读回应。限额照
//! 状态码（429、402 余额不足）和原话认（[`Failure::from_text`]）。

use std::time::Duration;

use reqwest::{Client, StatusCode};

use super::Failure;
use super::sources::Rest;
use crate::body;

/// 一份回应至多读几个字节。
const MOST_BYTES: usize = 4 * 1024 * 1024;

/// 发一次，交回回应的字。2xx 以外的：回的是 JSON 的照样交回（几家把「没额度」写在 JSON 里，读法认），别的当失败。
pub(super) async fn send(
    client: &Client,
    rest: &Rest,
    timeout: Duration,
) -> Result<String, Failure> {
    let request = if rest.get {
        client.get(&rest.url)
    } else {
        client.post(&rest.url)
    };
    let mut request = request.timeout(timeout);
    for (name, value) in &rest.headers {
        request = request.header(*name, value);
    }
    if let Some(body) = &rest.body {
        request = request
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(serde_json::to_vec(body).unwrap_or_default());
    }
    let response = request
        .send()
        .await
        .map_err(|error| Failure::Failed(error.without_url().to_string()))?;
    let status = response.status();
    if status == StatusCode::TOO_MANY_REQUESTS || status == StatusCode::PAYMENT_REQUIRED {
        return Err(Failure::Limited);
    }
    let bytes = body::read_prefix(response, MOST_BYTES)
        .await
        .map_err(|_| Failure::Failed("the answer broke off".to_string()))?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if !status.is_success() && !text.trim_start().starts_with('{') {
        return Err(Failure::from_text(&format!(
            "HTTP {}: {}",
            status.as_u16(),
            text.chars().take(200).collect::<String>()
        )));
    }
    Ok(text)
}
