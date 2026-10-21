//! 网址的服务登录（施工 X-42 上，`docs/blueprint/mcp.md`「待定（施工 X-42）」）：照 MCP 规范 2026-07-28 的 Authorization
//! （OAuth 2.1、RFC 9728 受保护资源元数据、RFC 8414 / OIDC 发现、RFC 8707 `resource`、RFC 9207 `iss`、RFC 7636 PKCE）。
//! 这里是不碰网络的那一半：读 `WWW-Authenticate`、规范网址、两种 well-known 的先后、校验授权服务器的元数据、PKCE、拼授权页、
//! 读回调、判 `iss`、挑 `scope`。

mod callback;
mod flow;

pub use callback::{Listener, Reply, listen};
pub use flow::{Discovered, Exchange, Registered, Tokens, Trouble, discover, exchange, register};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::Value;
use sha2::{Digest, Sha256};
use url::Url;

/// 受保护资源元数据的 well-known 后缀（RFC 9728）。
const RESOURCE_WELL_KNOWN: &str = "/.well-known/oauth-protected-resource";

/// 授权服务器元数据的 well-known 后缀（RFC 8414），和 OIDC 的（OpenID Connect Discovery 1.0）。
const OAUTH_WELL_KNOWN: &str = "/.well-known/oauth-authorization-server";
const OIDC_WELL_KNOWN: &str = "/.well-known/openid-configuration";

/// 回调的路径。
pub const CALLBACK_PATH: &str = "/callback";

/// 随机串的字节数：PKCE 的 `code_verifier`、`state` 都是 32 个字节，base64url 以后 43 个字。
const RANDOM_BYTES: usize = 32;

/// `401` 的 `WWW-Authenticate` 里 `Bearer` 那一段的几格。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Challenge {
    /// 受保护资源元数据在哪（`resource_metadata`）。
    pub resource_metadata: Option<String>,
    /// 要的范围（`scope`），空格分开。
    pub scope: Option<String>,
    /// 出了什么错（`error`，例如 `insufficient_scope`）。
    pub error: Option<String>,
}

/// 读 `WWW-Authenticate`：只认 `Bearer` 那一段（方案名不分大小写）；没有的没有。参数照 RFC 7235：`名字=记号` 或者
/// `名字="带引号的"`（里面的 `\` 转义下一个字），逗号分开；几种方案并在一起的，`Bearer` 后面到下一个方案名为止。
pub fn challenge(header: &str) -> Option<Challenge> {
    let mut rest = header.trim();
    let mut bearer: Option<Challenge> = None;
    while !rest.is_empty() {
        rest = rest.trim_start_matches([',', ' ', '\t']);
        let token_end = rest
            .find(|c: char| c == '=' || c == ',' || c.is_whitespace())
            .unwrap_or(rest.len());
        let (token, after) = rest.split_at(token_end);
        let after_space = after.trim_start();
        if token.is_empty() {
            break;
        }
        if let Some(given) = after_space.strip_prefix('=') {
            // 一个参数：属于当前的方案。
            let (value, more) = param_value(given.trim_start());
            if let Some(found) = bearer.as_mut() {
                let field = match token.to_ascii_lowercase().as_str() {
                    "resource_metadata" => Some(&mut found.resource_metadata),
                    "scope" => Some(&mut found.scope),
                    "error" => Some(&mut found.error),
                    _ => None,
                };
                if let Some(field) = field {
                    *field = Some(value);
                }
            }
            rest = more;
        } else {
            // 一个方案名：`Bearer` 开一段，别的方案的参数不收。
            if bearer.is_some() && !token.eq_ignore_ascii_case("bearer") {
                break;
            }
            if token.eq_ignore_ascii_case("bearer") && bearer.is_none() {
                bearer = Some(Challenge::default());
            }
            rest = after;
        }
    }
    bearer
}

/// 参数的值：带引号的到配对的引号为止（`\` 转义下一个字），别的到逗号、空白为止；交回值和后面剩下的。
fn param_value(text: &str) -> (String, &str) {
    if let Some(quoted) = text.strip_prefix('"') {
        let mut value = String::new();
        let mut chars = quoted.char_indices();
        while let Some((at, c)) = chars.next() {
            match c {
                '\\' => {
                    if let Some((_, next)) = chars.next() {
                        value.push(next);
                    }
                }
                '"' => return (value, &quoted[at + 1..]),
                other => value.push(other),
            }
        }
        return (value, "");
    }
    let end = text
        .find(|c: char| c == ',' || c.is_whitespace())
        .unwrap_or(text.len());
    (text[..end].to_string(), &text[end..])
}

/// 服务的规范网址（RFC 8707、规范的 Canonical Server URI）：scheme、主机小写，默认端口不写，去掉 fragment 和末尾的 `/`。
/// 不是 `http`、`https` 的网址没有。
pub fn canonical(url: &str) -> Option<String> {
    let mut parsed = Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return None;
    }
    parsed.set_fragment(None);
    let mut text = parsed.to_string();
    if parsed.query().is_none() && text.ends_with('/') {
        text.pop();
    }
    Some(text)
}

/// 受保护资源元数据的 well-known 网址，照规范的先后：服务网址带路径的先插在路径前面，再根上的。网址不对的没有。
pub fn resource_metadata_urls(server: &str) -> Vec<String> {
    let Some((origin, path)) = origin_and_path(server) else {
        return Vec::new();
    };
    let mut urls = Vec::new();
    if !path.is_empty() {
        urls.push(format!("{origin}{RESOURCE_WELL_KNOWN}{path}"));
    }
    urls.push(format!("{origin}{RESOURCE_WELL_KNOWN}"));
    urls
}

/// 授权服务器元数据的 well-known 网址，照规范的先后：`issuer` 带路径的三种（OAuth 插在路径前、OIDC 插在路径前、OIDC 接在
/// 路径后），不带的两种（OAuth、OIDC）。网址不对的没有。
pub fn authorization_metadata_urls(issuer: &str) -> Vec<String> {
    let Some((origin, path)) = origin_and_path(issuer) else {
        return Vec::new();
    };
    match path.is_empty() {
        true => vec![
            format!("{origin}{OAUTH_WELL_KNOWN}"),
            format!("{origin}{OIDC_WELL_KNOWN}"),
        ],
        false => vec![
            format!("{origin}{OAUTH_WELL_KNOWN}{path}"),
            format!("{origin}{OIDC_WELL_KNOWN}{path}"),
            format!("{origin}{path}{OIDC_WELL_KNOWN}"),
        ],
    }
}

/// 网址的 `scheme://主机[:端口]` 和路径（去掉末尾的 `/`，根上的是空的）。
fn origin_and_path(url: &str) -> Option<(String, String)> {
    let parsed = Url::parse(url).ok()?;
    parsed.host_str()?;
    let origin = parsed.origin().ascii_serialization();
    let path = parsed.path().trim_end_matches('/').to_string();
    Some((origin, path))
}

/// 授权服务器的元数据：只留要用的几格。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authorization {
    /// `issuer`，和拼网址用的一样。
    pub issuer: String,
    /// 授权页。
    pub authorization_endpoint: String,
    /// 换令牌。
    pub token_endpoint: String,
    /// 动态注册（RFC 7591），没有的没有。
    pub registration_endpoint: Option<String>,
    /// 认的范围。
    pub scopes_supported: Vec<String>,
    /// 授权的回应带 `iss`（RFC 9207）。
    pub iss_supported: bool,
}

/// 授权服务器的元数据不能用的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// `issuer` 和拼网址用的不一样。
    Issuer,
    /// 不认 PKCE 的 `S256`。
    NoS256,
    /// 几个端点不是 `https`（本机的例外）。
    Insecure,
    /// 少了一格。
    Missing(&'static str),
    /// 受保护资源元数据写的 `resource` 不是这个服务、也不是它的上一级。
    Resource,
}

/// 读授权服务器的元数据：`issuer` 必须和拼网址用的 `issuer` 一模一样（RFC 8414 第 3.3 节）；`code_challenge_methods_supported`
/// 里要有 `S256`，没写的当不支持、不降级；授权页、换令牌、注册几个端点只认 `https`（本机的地址例外，给测试）。
///
/// # Errors
///
/// 见 [`Problem`]。
pub fn authorization(doc: &Value, issuer: &str) -> Result<Authorization, Problem> {
    let text = |key: &'static str| doc.get(key).and_then(Value::as_str).map(str::to_string);
    let list = |key: &str| -> Vec<String> {
        doc.get(key)
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };
    if text("issuer").as_deref() != Some(issuer) {
        return Err(Problem::Issuer);
    }
    if !list("code_challenge_methods_supported")
        .iter()
        .any(|method| method == "S256")
    {
        return Err(Problem::NoS256);
    }
    let authorization_endpoint =
        text("authorization_endpoint").ok_or(Problem::Missing("authorization_endpoint"))?;
    let token_endpoint = text("token_endpoint").ok_or(Problem::Missing("token_endpoint"))?;
    let registration_endpoint = text("registration_endpoint");
    let secure = |url: &str| url.starts_with("https://") || miyu_http::is_loopback_url(url);
    if ![
        Some(&authorization_endpoint),
        Some(&token_endpoint),
        registration_endpoint.as_ref(),
    ]
    .into_iter()
    .flatten()
    .all(|url| secure(url))
    {
        return Err(Problem::Insecure);
    }
    Ok(Authorization {
        issuer: issuer.to_string(),
        authorization_endpoint,
        token_endpoint,
        registration_endpoint,
        scopes_supported: list("scopes_supported"),
        iss_supported: doc
            .get("authorization_response_iss_parameter_supported")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// PKCE 的 `S256`（RFC 7636 第 4.2 节）：`code_verifier` 的 SHA-256，base64url、不补 `=`。
pub fn challenge_of(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// 32 个随机字节的 base64url。
fn random() -> Result<String, getrandom::Error> {
    let mut bytes = [0u8; RANDOM_BYTES];
    getrandom::fill(&mut bytes)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// 一次登录的 PKCE。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkce {
    /// `code_verifier`：换令牌时交。
    pub verifier: String,
    /// `code_challenge`：授权页上带。
    pub challenge: String,
}

impl Pkce {
    /// 新的一对：`code_verifier` 是 32 个随机字节的 base64url（43 个字，RFC 7636 要 43 到 128 个）。
    ///
    /// # Errors
    ///
    /// 系统给不出随机数。
    pub fn new() -> Result<Pkce, getrandom::Error> {
        let verifier = random()?;
        let challenge = challenge_of(&verifier);
        Ok(Pkce {
            verifier,
            challenge,
        })
    }
}

/// 一次登录的 `state`：32 个随机字节的 base64url，一次一用。
///
/// # Errors
///
/// 系统给不出随机数。
pub fn random_state() -> Result<String, getrandom::Error> {
    random()
}

/// 拼授权页要的几样。
#[derive(Debug, Clone, Copy)]
pub struct Asked<'a> {
    /// 客户端编号。
    pub client_id: &'a str,
    /// 回调的地址。
    pub redirect_uri: &'a str,
    /// 要的范围，没有的不写。
    pub scope: Option<&'a str>,
    /// 服务的规范网址。
    pub resource: &'a str,
    /// 这一次的 `state`。
    pub state: &'a str,
    /// PKCE 的 `code_challenge`。
    pub challenge: &'a str,
}

/// 授权页的网址：授权服务器的授权页（它自己带的查询串照留）接上 `response_type=code`、`client_id`、`redirect_uri`、PKCE、
/// `state`、`resource`，有要的范围的接 `scope`。授权页的网址读不成的照原样接（元数据读的时候查过是网址）。
pub fn authorize_url(authorization: &Authorization, asked: &Asked<'_>) -> String {
    let Ok(mut url) = Url::parse(&authorization.authorization_endpoint) else {
        return authorization.authorization_endpoint.clone();
    };
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("response_type", "code")
            .append_pair("client_id", asked.client_id)
            .append_pair("redirect_uri", asked.redirect_uri)
            .append_pair("code_challenge", asked.challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", asked.state)
            .append_pair("resource", asked.resource);
        if let Some(scope) = asked.scope {
            query.append_pair("scope", scope);
        }
    }
    url.to_string()
}

/// 回调里读出来的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Callback {
    /// 同意了：授权码、`state`、`iss`。
    Code {
        /// 授权码。
        code: String,
        /// 交回来的 `state`。
        state: String,
        /// 授权服务器带的 `iss`。
        iss: Option<String>,
    },
    /// 出错了（人拒了这一类）。
    Error {
        /// `error`。
        error: String,
        /// `error_description`。
        description: Option<String>,
        /// 交回来的 `state`。
        state: Option<String>,
        /// 授权服务器带的 `iss`。
        iss: Option<String>,
    },
}

/// 读回调请求的目标（路径加查询串）：路径是 [`CALLBACK_PATH`]，有 `code` 和 `state` 的是同意了，有 `error` 的是出错了；别的
/// （浏览器顺手要的 `/favicon.ico`、没有 `state` 的）没有。
pub fn callback(target: &str) -> Option<Callback> {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != CALLBACK_PATH {
        return None;
    }
    let pairs: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();
    let get = |key: &str| {
        pairs
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
    };
    match (get("code"), get("error"), get("state")) {
        (Some(code), None, Some(state)) => Some(Callback::Code {
            code,
            state,
            iss: get("iss"),
        }),
        (_, Some(error), state) => Some(Callback::Error {
            error,
            description: get("error_description"),
            state,
            iss: get("iss"),
        }),
        _ => None,
    }
}

/// 照 RFC 9207 那张表判授权回应里的 `iss`（规范的 Authorization Response Validation）：带了的照原样和记下的 `issuer` 比（不改
/// 大小写、不去末尾的 `/`）；没带的，授权服务器说了会带（`supported`）的拒，没说的照过。
pub fn iss_ok(supported: bool, iss: Option<&str>, recorded: &str) -> bool {
    match iss {
        Some(iss) => iss == recorded,
        None => !supported,
    }
}

/// 第一次登录要的范围（规范的 Scope Selection Strategy）：`WWW-Authenticate` 里写了的照它，没写的照受保护资源元数据的
/// `scopes_supported`，都没有的不写。有了范围、授权服务器又认 `offline_access` 的接上它（要 `refresh_token`）；本来没有范围的
/// 不单写它：只写它会丢了授权服务器默认给的范围。
pub fn scope(
    challenged: Option<&str>,
    resource_scopes: &[String],
    authorization_scopes: &[String],
) -> Option<String> {
    let mut scopes: Vec<String> = match challenged {
        Some(challenged) => challenged.split_whitespace().map(str::to_string).collect(),
        None => resource_scopes.to_vec(),
    };
    if scopes.is_empty() {
        return None;
    }
    if authorization_scopes
        .iter()
        .any(|scope| scope == "offline_access")
        && !scopes.iter().any(|scope| scope == "offline_access")
    {
        scopes.push("offline_access".to_string());
    }
    Some(scopes.join(" "))
}

#[cfg(test)]
mod tests;
