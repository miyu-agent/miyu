//! 假的授权服务器（施工 X-42 上）：本机一个端口上照 OAuth 2.1 演授权服务器，测试自己当浏览器（[`browse`]）去点同意。
//!
//! - `GET /.well-known/oauth-authorization-server`：元数据，`issuer` 是它自己的网址，认 `S256`、带 `iss`。
//! - `POST /register`（JSON）：动态注册，发 `client-1`、`client-2`……，记下登记的 `redirect_uris`。
//! - `GET /authorize`：客户端、回调地址对得上的跳回回调（`code`、`state`、`iss`）；回调地址不对的照 OAuth 不跳，回 `400`。宽松的
//!   （[`AuthServer::start`]）只比到端口以前（RFC 8252 第 7.3 节），要精确的（[`AuthServer::exact_port`]）连端口一起比。
//! - `POST /token`（表单）：授权码换令牌，`code_verifier` 照 `S256` 对得上 `code_challenge`、回调地址、`resource`、客户端都和授权
//!   时一样才给 `access-1` 这样的令牌（带 `refresh_token`、一小时），不对的回 `400` `invalid_grant`。
//!
//! 每个回应都带 `Connection: close`。

use std::sync::{Arc, Mutex, PoisonError};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use url::Url;

/// 一个在跑的假授权服务器；丢掉就停。
pub struct AuthServer {
    /// 它的 `issuer`：`http://127.0.0.1:<端口>`。
    pub issuer: String,
    state: Arc<State>,
    task: tokio::task::JoinHandle<()>,
}

/// 授权时记下的一个授权码。
struct Granted {
    code: String,
    client_id: String,
    redirect_uri: String,
    challenge: String,
    resource: String,
}

struct State {
    issuer: String,
    exact_port: bool,
    /// 发出去的客户端：编号和登记的回调地址。
    clients: Mutex<Vec<(String, Vec<String>)>>,
    granted: Mutex<Vec<Granted>>,
    /// 收到的请求：方法和路径（带查询串）。
    received: Mutex<Vec<(String, String)>>,
    issued: Mutex<u32>,
    /// 换令牌时收到的 `code_verifier`：测试查运行日志里搜不到它。
    verifiers: Mutex<Vec<String>>,
    /// 给的令牌几秒过期（施工 X-42 下）：没改的一小时。
    lifetime: std::sync::atomic::AtomicU64,
    /// 续的时候拒（施工 X-42 下）。
    refuse_refresh: std::sync::atomic::AtomicBool,
    /// 续过的几次：带的 `resource`、`scope`（施工 X-42 下）。
    refreshes: Mutex<Vec<(String, String)>>,
}

impl AuthServer {
    /// 起一个宽松的：回调地址比到端口以前。
    ///
    /// # Panics
    ///
    /// 本机的端口开不了。
    pub async fn start() -> AuthServer {
        AuthServer::open(false).await
    }

    /// 起一个要精确的：回调地址连端口一起比（不守 RFC 8252 第 7.3 节的那种）。
    ///
    /// # Panics
    ///
    /// 本机的端口开不了。
    pub async fn exact_port() -> AuthServer {
        AuthServer::open(true).await
    }

    async fn open(exact_port: bool) -> AuthServer {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("本机的端口开得了");
        let issuer = format!("http://{}", listener.local_addr().expect("有地址"));
        let state = Arc::new(State {
            issuer: issuer.clone(),
            exact_port,
            clients: Mutex::new(Vec::new()),
            granted: Mutex::new(Vec::new()),
            received: Mutex::new(Vec::new()),
            issued: Mutex::new(0),
            verifiers: Mutex::new(Vec::new()),
            lifetime: std::sync::atomic::AtomicU64::new(3600),
            refuse_refresh: std::sync::atomic::AtomicBool::new(false),
            refreshes: Mutex::new(Vec::new()),
        });
        let serving = Arc::clone(&state);
        let task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(answer(stream, Arc::clone(&serving)));
            }
        });
        AuthServer {
            issuer,
            state,
            task,
        }
    }

    /// 之后给的令牌 `seconds` 秒就过期（施工 X-42 下）。
    pub fn expire_after(&self, seconds: u64) {
        self.state
            .lifetime
            .store(seconds, std::sync::atomic::Ordering::Relaxed);
    }

    /// 之后续的时候都拒，回 `invalid_grant`（施工 X-42 下）。
    pub fn refuse_refresh(&self) {
        self.state
            .refuse_refresh
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// 续过的几次：带的 `resource`、`scope`，照先后（施工 X-42 下）。
    pub fn refreshes(&self) -> Vec<(String, String)> {
        self.state
            .refreshes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 收到的请求：方法和路径（带查询串），照先后。
    pub fn received(&self) -> Vec<(String, String)> {
        self.state
            .received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 换令牌时收到的 `code_verifier`，照先后。
    pub fn verifiers(&self) -> Vec<String> {
        self.state
            .verifiers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 动态注册过几次。
    pub fn registrations(&self) -> usize {
        self.state
            .clients
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

impl Drop for AuthServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// 测试当浏览器：`GET` 授权页（只认 `http://127.0.0.1`），跳的照 `Location` 再 `GET` 一次（回调），交回最后那一页的状态和正文。
/// 不跳的（回调地址不对）交回授权页的状态。
///
/// # Panics
///
/// 网址不是 `http://127.0.0.1:<端口>/…`。
pub async fn browse(url: &str) -> (u16, String) {
    let (status, location, body) = get(url).await;
    match location {
        Some(next) if (300..400).contains(&status) => {
            let (status, _, body) = get(&next).await;
            (status, body)
        }
        _ => (status, body),
    }
}

/// 打开授权页、不跟着跳：交回它要跳去的回调地址（施工 X-42 下：手贴回调，浏览器在别的机器上的那一种）。
///
/// # Panics
///
/// 连不上、写不出去。
pub async fn redirect_of(url: &str) -> Option<String> {
    let (status, location, _) = get(url).await;
    location.filter(|_| (300..400).contains(&status))
}

/// 一次 `GET`：状态、`Location`、正文。
async fn get(url: &str) -> (u16, Option<String>, String) {
    let parsed = Url::parse(url).expect("是网址");
    let address = format!(
        "{}:{}",
        parsed.host_str().expect("有主机"),
        parsed.port().expect("有端口")
    );
    let target = match parsed.query() {
        Some(query) => format!("{}?{query}", parsed.path()),
        None => parsed.path().to_string(),
    };
    let mut stream = TcpStream::connect(&address).await.expect("连得上");
    let request = format!("GET {target} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .await
        .expect("写得出去");
    let mut raw = String::new();
    drop(stream.read_to_string(&mut raw).await);
    let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((raw.as_str(), ""));
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    let location = head.lines().find_map(|line| {
        line.split_once(':')
            .filter(|(name, _)| name.eq_ignore_ascii_case("location"))
            .map(|(_, value)| value.trim().to_string())
    });
    (status, location, body.to_string())
}

/// 答一条连接上的一个请求。
async fn answer(stream: TcpStream, state: Arc<State>) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).await.is_err() {
        return;
    }
    let mut words = line.split_whitespace();
    let method = words.next().unwrap_or_default().to_string();
    let target = words.next().unwrap_or_default().to_string();
    let mut length = 0;
    loop {
        line.clear();
        if reader.read_line(&mut line).await.unwrap_or(0) == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.trim_end().split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0; length];
    if reader.read_exact(&mut body).await.is_err() {
        return;
    }
    state
        .received
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push((method.clone(), target.clone()));
    let (status, extra, text) = reply(&state, &method, &target, &body);
    let head = format!(
        "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n{extra}\r\n",
        text.len()
    );
    let mut stream = reader.into_inner();
    if stream.write_all(head.as_bytes()).await.is_ok()
        && stream.write_all(text.as_bytes()).await.is_ok()
    {
        drop(stream.shutdown().await);
    }
}

/// 照请求答：状态、另加的头、正文。
fn reply(state: &State, method: &str, target: &str, body: &[u8]) -> (u16, String, String) {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    match (method, path) {
        ("GET", "/.well-known/oauth-authorization-server") => {
            let issuer = &state.issuer;
            let metadata = json!({
                "issuer": issuer,
                "authorization_endpoint": format!("{issuer}/authorize"),
                "token_endpoint": format!("{issuer}/token"),
                "registration_endpoint": format!("{issuer}/register"),
                "scopes_supported": ["files:read", "offline_access"],
                "code_challenge_methods_supported": ["S256"],
                "authorization_response_iss_parameter_supported": true,
            });
            (200, String::new(), metadata.to_string())
        }
        ("POST", "/register") => register(state, body),
        ("GET", "/authorize") => authorize(state, query),
        ("POST", "/token") => token(state, body),
        _ => (404, String::new(), "{}".to_string()),
    }
}

/// 动态注册：`redirect_uris` 不空的发一个客户端。
fn register(state: &State, body: &[u8]) -> (u16, String, String) {
    let asked: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let uris: Vec<String> = asked["redirect_uris"]
        .as_array()
        .map(|uris| {
            uris.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if uris.is_empty() {
        return (
            400,
            String::new(),
            json!({"error": "invalid_redirect_uri"}).to_string(),
        );
    }
    let mut clients = state.clients.lock().unwrap_or_else(PoisonError::into_inner);
    let client_id = format!("client-{}", clients.len() + 1);
    clients.push((client_id.clone(), uris.clone()));
    (
        201,
        String::new(),
        json!({"client_id": client_id, "redirect_uris": uris, "token_endpoint_auth_method": "none"}).to_string(),
    )
}

/// 授权：客户端、回调地址对得上的跳回回调；不对的不跳。
fn authorize(state: &State, query: &str) -> (u16, String, String) {
    let pairs: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();
    let get = |key: &str| {
        pairs
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
            .unwrap_or_default()
    };
    let (client_id, redirect_uri) = (get("client_id"), get("redirect_uri"));
    let registered = state
        .clients
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .find(|(id, _)| *id == client_id)
        .map(|(_, uris)| uris.clone());
    let Some(registered) = registered else {
        return (
            400,
            String::new(),
            json!({"error": "invalid_client"}).to_string(),
        );
    };
    let fits = |known: &str| match state.exact_port {
        true => known == redirect_uri,
        false => without_port(known) == without_port(&redirect_uri),
    };
    if !registered.iter().any(|known| fits(known)) || get("code_challenge_method") != "S256" {
        return (
            400,
            String::new(),
            json!({"error": "invalid_request"}).to_string(),
        );
    }
    let mut granted = state.granted.lock().unwrap_or_else(PoisonError::into_inner);
    let code = format!("code-{}", granted.len() + 1);
    granted.push(Granted {
        code: code.clone(),
        client_id,
        redirect_uri: redirect_uri.clone(),
        challenge: get("code_challenge"),
        resource: get("resource"),
    });
    let mut back = Url::parse(&redirect_uri).expect("回调是网址");
    back.query_pairs_mut()
        .append_pair("code", &code)
        .append_pair("state", &get("state"))
        .append_pair("iss", &state.issuer);
    (302, format!("Location: {back}\r\n"), String::new())
}

/// 去掉端口的回调地址：去不掉的照原样。
fn without_port(uri: &str) -> String {
    if let Ok(mut parsed) = Url::parse(uri)
        && parsed.set_port(None).is_ok()
    {
        return parsed.to_string();
    }
    uri.to_string()
}

/// 换令牌：授权码、`code_verifier`、回调地址、`resource`、客户端都对得上才给。
fn token(state: &State, body: &[u8]) -> (u16, String, String) {
    let pairs: Vec<(String, String)> = url::form_urlencoded::parse(body).into_owned().collect();
    let get = |key: &str| {
        pairs
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
            .unwrap_or_default()
    };
    state
        .verifiers
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(get("code_verifier"));
    let refused = (
        400,
        String::new(),
        json!({"error": "invalid_grant"}).to_string(),
    );
    if get("grant_type") == "refresh_token" {
        return refresh(state, &get, refused);
    }
    if get("grant_type") != "authorization_code" {
        return refused;
    }
    let mut granted = state.granted.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(at) = granted.iter().position(|one| one.code == get("code")) else {
        return refused;
    };
    let one = granted.remove(at);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(get("code_verifier").as_bytes()));
    if challenge != one.challenge
        || get("redirect_uri") != one.redirect_uri
        || get("resource") != one.resource
        || get("client_id") != one.client_id
    {
        return refused;
    }
    issue(state)
}

/// 续（施工 X-42 下）：`refresh_token` 是发过的那一个、客户端对得上才给新的一对；拒的照 `refuse_refresh`。
fn refresh(
    state: &State,
    get: &dyn Fn(&str) -> String,
    refused: (u16, String, String),
) -> (u16, String, String) {
    if state
        .refuse_refresh
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        return refused;
    }
    let issued = *state.issued.lock().unwrap_or_else(PoisonError::into_inner);
    let known = (1..=issued).any(|n| get("refresh_token") == format!("refresh-{n}"));
    let client = state
        .clients
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .any(|(id, _)| *id == get("client_id"));
    if !known || !client {
        return refused;
    }
    state
        .refreshes
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push((get("resource"), get("scope")));
    issue(state)
}

/// 发新的一对令牌：`access-<n>`、`refresh-<n>`，照 `lifetime` 过期。
fn issue(state: &State) -> (u16, String, String) {
    let mut issued = state.issued.lock().unwrap_or_else(PoisonError::into_inner);
    *issued += 1;
    let tokens = json!({
        "access_token": format!("access-{issued}"),
        "token_type": "Bearer",
        "expires_in": state.lifetime.load(std::sync::atomic::Ordering::Relaxed),
        "refresh_token": format!("refresh-{issued}"),
        "scope": "files:read offline_access",
    });
    (200, String::new(), tokens.to_string())
}
