//! 后台页服务的路由（施工 BP-3，`package-pages.md`「不靠网页打开软件后台」第 3 条）：一个个接连接、照路径答；闲够了就关。
//!
//! - `GET /h/<票>/`、`host.js`、`host.css`：宿主页（资源目录的 `core/backstage/`）。
//! - `GET /p/<票>/<编号>/<路径>`：后台页的文件，编号要是票认的那个；路径照 `package.file` 的规矩，以 `/` 结尾的补 `index.html`。
//! - `POST /r/<票>`：宿主页替框调核心（`bridge.rs`），正文最多 1 MiB。`GET /e/<票>`：推送流（`events.rs`）。
//! - `Host` 头要是 `127.0.0.1:<端口>` 或 `localhost:<端口>`（挡借别的域名指过来的网页）；票不认识的 404，方法不对的 405。

use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::combinators::UnsyncBoxBody;
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::Incoming;
use hyper::header::{self, HeaderValue};
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::Value;

use super::{Running, TARGET, Ticket, bridge, events};

type Body = UnsyncBoxBody<Bytes, Infallible>;

const HOST_HTML: &str = include_str!("../../../../../resources/core/backstage/host.html");
const HOST_JS: &str = include_str!("../../../../../resources/core/backstage/host.js");
const HOST_CSS: &str = include_str!("../../../../../resources/core/backstage/host.css");

/// 宿主页的内容安全策略：只取自己的脚本、样式，只连自己，只嵌同源的框，不许被别人嵌。
const HOST_CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; frame-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

/// 后台页的内容安全策略（同网页的 `backstage_csp`）：沙箱、只取自己的文件、不许联网、只许嵌在同源的宿主页里。
const PAGE_CSP: &str = "sandbox allow-scripts allow-forms; default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'none'; frame-ancestors 'self'";

/// 宿主页一次请求的正文最多多大。
const BODY_MAX: usize = 1024 * 1024;

/// 后台页一份文件最多多大：页面的脚本、图都在这以内。
const FILE_MAX: u64 = 32 * 1024 * 1024;

/// 一个个接连接，闲够了就关。
pub(super) async fn run(listener: tokio::net::TcpListener, running: Arc<Running>, idle: Duration) {
    // 在循环外面计时：接连接不重新算，连接来得再密也照样查闲没闲。
    let mut tick =
        tokio::time::interval((idle / 4).clamp(Duration::from_millis(10), Duration::from_secs(30)));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                if let Ok((stream, _)) = accepted {
                    tokio::spawn(connection(stream, Arc::clone(&running)));
                }
            }
            _ = tick.tick() => {
                if running.idle(idle) || running.core.strong_count() == 0 {
                    running.close();
                    tracing::info!(target: TARGET, port = running.port, "page host closed");
                    return;
                }
            }
        }
    }
}

async fn connection(stream: tokio::net::TcpStream, running: Arc<Running>) {
    let service = hyper::service::service_fn(move |request| {
        let running = Arc::clone(&running);
        async move { Ok::<_, Infallible>(answer(request, &running).await) }
    });
    drop(
        hyper::server::conn::http1::Builder::new()
            .serve_connection(TokioIo::new(stream), service)
            .await,
    );
}

async fn answer(request: Request<Incoming>, running: &Arc<Running>) -> Response<Body> {
    running.touch();
    if !from_here(&request, running.port) {
        return status(StatusCode::MISDIRECTED_REQUEST);
    }
    let path = request.uri().path().to_string();
    let mut parts = path.trim_start_matches('/').splitn(3, '/');
    let (kind, ticket, rest) = (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
        parts.next(),
    );
    let Some(found) = running.ticket(ticket) else {
        return status(StatusCode::NOT_FOUND);
    };
    let get = request.method() == Method::GET;
    match (kind, rest) {
        ("h", Some(file)) if get => host_file(file),
        ("p", Some(rest)) if get => page_file(running, &found, rest).await,
        ("r", None) if request.method() == Method::POST => call(running, &found, request).await,
        ("e", None) if get => stream(running, &found).await,
        ("h" | "p", Some(_)) | ("r" | "e", None) => status(StatusCode::METHOD_NOT_ALLOWED),
        _ => status(StatusCode::NOT_FOUND),
    }
}

/// `Host` 头是本机这个端口。
fn from_here(request: &Request<Incoming>, port: u16) -> bool {
    request
        .headers()
        .get(header::HOST)
        .and_then(|host| host.to_str().ok())
        .is_some_and(|host| {
            host == format!("127.0.0.1:{port}") || host == format!("localhost:{port}")
        })
}

fn host_file(file: &str) -> Response<Body> {
    let (body, kind) = match file {
        "" | "index.html" => (HOST_HTML, "text/html; charset=utf-8"),
        "host.js" => (HOST_JS, "text/javascript; charset=utf-8"),
        "host.css" => (HOST_CSS, "text/css; charset=utf-8"),
        _ => return status(StatusCode::NOT_FOUND),
    };
    let mut response = full(Bytes::from_static(body.as_bytes()), kind);
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(HOST_CSP),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

async fn page_file(running: &Running, ticket: &Ticket, rest: &str) -> Response<Body> {
    let Some(core) = running.core.upgrade() else {
        return status(StatusCode::SERVICE_UNAVAILABLE);
    };
    let (package, path) = rest.split_once('/').unwrap_or((rest, ""));
    let Some(path) = decoded(path) else {
        return status(StatusCode::NOT_FOUND);
    };
    if decoded(package).as_deref() != Some(ticket.package.as_str()) {
        return status(StatusCode::NOT_FOUND);
    }
    let path = match path.strip_suffix('/') {
        Some(dir) if !dir.is_empty() => format!("{dir}/index.html"),
        _ => path,
    };
    let (Ok(dir), Some(segments)) = (
        super::super::file::page_dir(&core, &ticket.package),
        super::super::segments(&path),
    ) else {
        return status(StatusCode::NOT_FOUND);
    };
    let kind = media_type(segments.last().map_or("", String::as_str));
    let read = tokio::task::spawn_blocking(move || read_file(&dir, &segments)).await;
    let Ok(Some(bytes)) = read else {
        return status(StatusCode::NOT_FOUND);
    };
    let mut response = full(Bytes::from(bytes), kind);
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(PAGE_CSP),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    response
}

fn read_file(dir: &std::path::Path, segments: &[String]) -> Option<Vec<u8>> {
    let real: PathBuf = super::super::beneath(dir, segments)?;
    let size = std::fs::metadata(&real).ok()?.len();
    (size <= FILE_MAX)
        .then(|| std::fs::read(&real).ok())
        .flatten()
}

async fn call(running: &Running, ticket: &Ticket, request: Request<Incoming>) -> Response<Body> {
    let Some(core) = running.core.upgrade() else {
        return status(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(body) = Limited::new(request.into_body(), BODY_MAX).collect().await else {
        return status(StatusCode::PAYLOAD_TOO_LARGE);
    };
    let Ok(message) = serde_json::from_slice::<Value>(&body.to_bytes()) else {
        return status(StatusCode::BAD_REQUEST);
    };
    let Ok(link) = ticket.link(&core).await else {
        return status(StatusCode::SERVICE_UNAVAILABLE);
    };
    let reply = bridge::answer(&link, ticket, &message).await;
    let mut response = full(Bytes::from(reply.to_string()), "application/json");
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn stream(running: &Arc<Running>, ticket: &Ticket) -> Response<Body> {
    let Some(core) = running.core.upgrade() else {
        return status(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(link) = ticket.link(&core).await else {
        return status(StatusCode::SERVICE_UNAVAILABLE);
    };
    let body = events::open(link, ticket.package.clone(), Arc::clone(running)).await;
    let mut response = Response::new(body.boxed_unsync());
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    secure(headers);
    response
}

/// 每个回应都带的：不猜类型、不带来源。从来不设 cookie。
fn secure(headers: &mut hyper::HeaderMap) {
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
}

fn full(body: Bytes, kind: &'static str) -> Response<Body> {
    let mut response = Response::new(Full::new(body).boxed_unsync());
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(kind));
    secure(headers);
    response
}

fn status(code: StatusCode) -> Response<Body> {
    let mut response = full(Bytes::new(), "text/plain; charset=utf-8");
    *response.status_mut() = code;
    response
}

/// 网址里的路径照 `%XX` 解开；写坏了的、解出来不是 UTF-8 的没有。
fn decoded(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let hex = std::str::from_utf8(bytes.get(at + 1..at + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// 照扩展名认类型（同网页 `web.json` 的那张表里常用的几种），认不出的 `application/octet-stream`。
fn media_type(name: &str) -> &'static str {
    let extension = name
        .rsplit_once('.')
        .map_or("", |(_, ext)| ext)
        .to_ascii_lowercase();
    match extension.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "txt" => "text/plain; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}
