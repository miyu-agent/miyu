//! 发出去、等到响应头（施工 V-38）：请求体成了流，reqwest 自己那两样重发拿不到请求体了（它只重放整份在内存里的请求体），
//! 照 reqwest 0.12.28 原来的条件、次数在这里补回来，每一次都从同一份片段清单从头再造一条流。
//!
//! - 307、308 跳转（原来是 tower-http 的 `FollowRedirect` 照 reqwest 的 `TowerRedirectPolicy` 做）：`Location` 照当前的地址
//!   解析，解析不了的照原样交回这个 3xx；最多跟 10 次，第 11 次报「error following redirect: too many redirects」；方法、
//!   请求体不变，头从原来的那一份起，跨了主机（主机名或端口不一样）的去掉 `Authorization`、`Cookie`、`Cookie2`、
//!   `Proxy-Authorization`、`WWW-Authenticate`，带上 `Referer`（上一个地址去掉用户名、密码、`#` 后面的；https 跳到 http 的
//!   不带）。地址不是 http、https 的，照 reqwest 当场报造不出请求。301、302、303 不用请求体（改成不带请求体的 GET），
//!   reqwest 照旧自己跟。
//! - 对方没处理就拒了（原来是 reqwest 默认的 `retry` 策略，`ProtocolNacks`）：h2 上对方发来 `REFUSED_STREAM` 的重置，或者
//!   `NO_ERROR` 的 `GOAWAY`（这个流在对方说的最后一个之后），立刻再发，最多再发 2 次；每一跳各算各的。HTTP/1 的连接在写请求
//!   之前就断了的，hyper 自己拿着没动过的请求换连接重发，流还没读过，照旧。
//!
//! 一跳里 reqwest 自己跟的 301、302、303 照它自己的十次算，和这里的十次分开数：只有几种跳转混着来才会不一样。

use std::error::Error;
use std::sync::Arc;

use reqwest::header::{
    AUTHORIZATION, COOKIE, HeaderMap, HeaderValue, LOCATION, PROXY_AUTHORIZATION, REFERER,
    WWW_AUTHENTICATE,
};
use reqwest::{Client, Request, Response, StatusCode, Url};

use crate::body::Replay;

/// 307、308 最多跟几次：reqwest 默认的 `Policy::limited(10)`。
pub(crate) const MAX_REDIRECTS: usize = 10;

/// 对方没处理就拒了的，一跳最多再发几次：reqwest 默认的 `max_retries_per_request`。
pub(crate) const NACK_RETRIES: u32 = 2;

/// 发出去怎么样了。
pub(crate) enum Delivered {
    /// 收到了响应头（最后一跳的）。
    Response(Response),
    /// 发不出去：reqwest 的错，照原来那样分类。
    Failed(reqwest::Error),
    /// 307、308 跳了十次还在跳：原话照 reqwest 的「error following redirect: too many redirects」。
    TooManyRedirects,
}

/// 照 `template`（不带请求体的那一份）发，请求体每次从 `replay` 从头造；跟 307、308，对方没处理的重发。
pub(crate) async fn deliver(client: &Client, template: Request, replay: &Arc<Replay>) -> Delivered {
    let original = template.headers().clone();
    let mut request = template;
    // 跳过的地址，照先后：reqwest 记的那一串。
    let mut previous: Vec<Url> = Vec::new();
    loop {
        let response = match send(client, &request, replay).await {
            Ok(response) => response,
            Err(error) => return Delivered::Failed(error),
        };
        if !matches!(
            response.status(),
            StatusCode::TEMPORARY_REDIRECT | StatusCode::PERMANENT_REDIRECT
        ) {
            return Delivered::Response(response);
        }
        let current = response.url().clone();
        let Some(next) = location(&response, &current) else {
            return Delivered::Response(response);
        };
        previous.push(current);
        // 第一个是最初的地址，不算跳转：多于十个就是第十一次了。
        if previous.len() > MAX_REDIRECTS {
            return Delivered::TooManyRedirects;
        }
        let mut headers = original.clone();
        if let Some(last) = previous.last() {
            if cross_host(&next, last) {
                for name in [AUTHORIZATION, COOKIE, PROXY_AUTHORIZATION, WWW_AUTHENTICATE] {
                    headers.remove(name);
                }
                headers.remove("cookie2");
            }
            if let Some(referer) = referer(&next, last) {
                headers.insert(REFERER, referer);
            }
        }
        request = again(&request, next, headers);
    }
}

/// 发一跳：对方没处理就拒了的立刻再发，最多再发 [`NACK_RETRIES`] 次。
async fn send(
    client: &Client,
    request: &Request,
    replay: &Arc<Replay>,
) -> reqwest::Result<Response> {
    let mut retries = 0;
    loop {
        let mut attempt = again(request, request.url().clone(), request.headers().clone());
        *attempt.body_mut() = Some(reqwest::Body::wrap(replay.stream()));
        match client.execute(attempt).await {
            Err(error) if retries < NACK_RETRIES && refused(&error) => retries += 1,
            done => return done,
        }
    }
}

/// 照 `request` 的方法、版本另造一份发到 `url`、带着 `headers`，不带请求体。
fn again(request: &Request, url: Url, headers: HeaderMap) -> Request {
    let mut next = Request::new(request.method().clone(), url);
    *next.headers_mut() = headers;
    *next.version_mut() = request.version();
    next
}

/// `Location` 照当前的地址解析；没有、不是字、解析不了、成不了 HTTP 的地址的没有（tower-http 的 `resolve_uri`）。
fn location(response: &Response, current: &Url) -> Option<Url> {
    let text = response.headers().get(LOCATION)?.to_str().ok()?;
    let next = current.join(text).ok()?;
    http::Uri::try_from(next.as_str()).ok()?;
    Some(next)
}

/// 跨没跨主机：主机名或者端口（没写的照协议的默认）不一样（reqwest 的 `remove_sensitive_headers`）。
fn cross_host(next: &Url, previous: &Url) -> bool {
    next.host_str() != previous.host_str()
        || next.port_or_known_default() != previous.port_or_known_default()
}

/// 带上的 `Referer`：上一个地址去掉用户名、密码、`#` 后面的；https 跳到 http 的不带（reqwest 的 `make_referer`）。
fn referer(next: &Url, previous: &Url) -> Option<HeaderValue> {
    if next.scheme() == "http" && previous.scheme() == "https" {
        return None;
    }
    let mut referer = previous.clone();
    // 去不掉的（没有主机的那种地址）照原样：reqwest 也是不管成不成。
    referer.set_username("").unwrap_or_default();
    referer.set_password(None).unwrap_or_default();
    referer.set_fragment(None);
    referer.as_str().parse().ok()
}

/// 对方没处理就拒了的（reqwest 的 `is_retryable_error`）：h2 上对方发来的 `REFUSED_STREAM` 重置，或者 `NO_ERROR` 的 `GOAWAY`。
fn refused(error: &reqwest::Error) -> bool {
    let mut source = error.source();
    while let Some(cause) = source {
        if let Some(h2) = cause.downcast_ref::<h2::Error>() {
            let go_away = h2.is_go_away() && h2.reason() == Some(h2::Reason::NO_ERROR);
            let refused = h2.is_reset() && h2.reason() == Some(h2::Reason::REFUSED_STREAM);
            return h2.is_remote() && (go_away || refused);
        }
        source = cause.source();
    }
    false
}
