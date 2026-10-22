//! 登录的那几次网络往来（施工 X-42 上）：取受保护资源元数据、授权服务器元数据（规范的 Authorization Server Discovery），动态注册
//! （RFC 7591，规范标成了旧办法、现在的服务多半还认），授权码换令牌，续期（施工 X-42 下）。授权服务器回的错误说明是不可信的字：
//! 截到 200 个字符。

use serde_json::{Value, json};

use miyu_http::Client;

use super::{
    Authorization, Challenge, Problem, authorization, authorization_metadata_urls, canonical,
    resource_metadata_urls, scope,
};

/// 授权服务器回的错误说明最多留几个字符。
const DESCRIPTION_LIMIT: usize = 200;

/// 发现的结果：给令牌的 `resource`、授权服务器、第一次登录要的范围。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovered {
    /// 换令牌、授权页上写的 `resource`（RFC 8707）：服务的规范网址，元数据写了上一级的照它。
    pub resource: String,
    /// 授权服务器。
    pub authorization: Authorization,
    /// 第一次登录要的范围，没有的不写。
    pub scope: Option<String>,
}

/// 一个客户端：编号，预先注册的、动态注册拿到的密钥（没有的没有）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registered {
    /// `client_id`。
    pub client_id: String,
    /// `client_secret`。
    pub client_secret: Option<String>,
}

/// 换来的令牌。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tokens {
    /// `access_token`。
    pub access_token: String,
    /// `refresh_token`，授权服务器没给的没有。
    pub refresh_token: Option<String>,
    /// 几秒以后到期，没说的没有。
    pub expires_in: Option<u64>,
    /// 实际给的范围，没说的没有。
    pub scope: Option<String>,
}

/// 登录这几步走不下去的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trouble {
    /// 连不上、回的不是 JSON 这一类：一句英文。
    Unreachable(String),
    /// 找不到受保护资源元数据，或者授权服务器的元数据。
    NoMetadata,
    /// 元数据不能用。
    Metadata(Problem),
    /// 受保护资源元数据里没有授权服务器。
    NoAuthorizationServer,
    /// 没有预先注册的客户端，授权服务器也不认动态注册。
    Unregistered,
    /// 授权服务器拒了：`error` 和截过的说明。
    Refused {
        /// `error`。
        error: String,
        /// `error_description`，截过。
        description: Option<String>,
    },
}

/// 照 `401` 的挑战 `challenge` 找服务 `server` 的授权服务器：受保护资源元数据先照挑战里写的，没写的照两个 well-known 依次找；
/// 授权服务器取 `authorization_servers` 的第一个，元数据照规范的先后找、照 [`authorization`] 查。
///
/// # Errors
///
/// 见 [`Trouble`]。
pub async fn discover(
    http: &Client,
    server: &str,
    challenge: &Challenge,
) -> Result<Discovered, Trouble> {
    let own = canonical(server).ok_or(Trouble::NoMetadata)?;
    let urls = match &challenge.resource_metadata {
        Some(url) => vec![url.clone()],
        None => resource_metadata_urls(server),
    };
    let resource = first_json(http, &urls).await?.ok_or(Trouble::NoMetadata)?;
    let issuer = resource["authorization_servers"]
        .as_array()
        .and_then(|servers| servers.iter().find_map(Value::as_str))
        .ok_or(Trouble::NoAuthorizationServer)?
        .to_string();
    let authorization = authorization_of(http, &issuer).await?;
    let resource_scopes: Vec<String> = resource["scopes_supported"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let target = match resource["resource"].as_str().and_then(canonical) {
        Some(named) if own == named || own.starts_with(&format!("{named}/")) => named,
        Some(_) => return Err(Trouble::Metadata(Problem::Resource)),
        None => own,
    };
    Ok(Discovered {
        resource: target,
        scope: scope(
            challenge.scope.as_deref(),
            &resource_scopes,
            &authorization.scopes_supported,
        ),
        authorization,
    })
}

/// 授权服务器 `issuer` 的元数据：照规范的先后找、照 [`authorization`] 查（续期时照存着的 `issuer` 再取一次，施工 X-42 下）。
///
/// # Errors
///
/// 找不到的 [`Trouble::NoMetadata`]；不能用的 [`Trouble::Metadata`]；连不上的 [`Trouble::Unreachable`]。
pub async fn authorization_of(http: &Client, issuer: &str) -> Result<Authorization, Trouble> {
    let found = first_json(http, &authorization_metadata_urls(issuer))
        .await?
        .ok_or(Trouble::NoMetadata)?;
    authorization(&found, issuer).map_err(Trouble::Metadata)
}

/// 动态注册一个客户端（RFC 7591）：本机的应用（`native`），不带密钥（`none`），回调只有 `redirect_uri` 这一个。
///
/// # Errors
///
/// 授权服务器不认动态注册的 [`Trouble::Unregistered`]；拒了的 [`Trouble::Refused`]；别的 [`Trouble::Unreachable`]。
pub async fn register(
    http: &Client,
    authorization: &Authorization,
    redirect_uri: &str,
) -> Result<Registered, Trouble> {
    let endpoint = authorization
        .registration_endpoint
        .as_deref()
        .ok_or(Trouble::Unregistered)?;
    let asked = json!({
        "client_name": "Miyu",
        "redirect_uris": [redirect_uri],
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none",
        "application_type": "native",
    });
    let answer = send(
        http.post(endpoint)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(asked.to_string()),
    )
    .await?;
    let client_id = answer["client_id"]
        .as_str()
        .ok_or_else(|| Trouble::Unreachable("registration without client_id".to_string()))?;
    Ok(Registered {
        client_id: client_id.to_string(),
        client_secret: answer["client_secret"].as_str().map(str::to_string),
    })
}

/// 换一次令牌要的几样。
#[derive(Debug, Clone, Copy)]
pub struct Exchange<'a> {
    /// 回调里拿到的授权码。
    pub code: &'a str,
    /// PKCE 的 `code_verifier`。
    pub verifier: &'a str,
    /// 授权页上写的回调地址。
    pub redirect_uri: &'a str,
    /// 授权页上写的 `resource`。
    pub resource: &'a str,
}

/// 授权码换令牌（`grant_type=authorization_code`），带 `code_verifier`、`resource`；有密钥的客户端照 `client_secret_post` 带上。
///
/// # Errors
///
/// 拒了的 [`Trouble::Refused`]；别的 [`Trouble::Unreachable`]。
pub async fn exchange(
    http: &Client,
    authorization: &Authorization,
    client: &Registered,
    asked: &Exchange<'_>,
) -> Result<Tokens, Trouble> {
    // 拼好就丢掉拼的那一个：它不能跨着 await 留着（不是 `Sync`）。
    let body = {
        let mut form = url::form_urlencoded::Serializer::new(String::new());
        form.append_pair("grant_type", "authorization_code")
            .append_pair("code", asked.code)
            .append_pair("redirect_uri", asked.redirect_uri)
            .append_pair("client_id", &client.client_id)
            .append_pair("code_verifier", asked.verifier)
            .append_pair("resource", asked.resource);
        if let Some(secret) = &client.client_secret {
            form.append_pair("client_secret", secret);
        }
        form.finish()
    };
    tokens(http, authorization, body).await
}

/// 照表单 `body` 向令牌端点要一对令牌：换令牌、续期共用。
async fn tokens(
    http: &Client,
    authorization: &Authorization,
    body: String,
) -> Result<Tokens, Trouble> {
    let answer = send(
        http.post(&authorization.token_endpoint)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Accept", "application/json")
            .body(body),
    )
    .await?;
    let access_token = answer["access_token"]
        .as_str()
        .ok_or_else(|| Trouble::Unreachable("token answer without access_token".to_string()))?;
    Ok(Tokens {
        access_token: access_token.to_string(),
        refresh_token: answer["refresh_token"].as_str().map(str::to_string),
        expires_in: answer["expires_in"].as_u64(),
        scope: answer["scope"].as_str().map(str::to_string),
    })
}

/// 续期一次要的几样（施工 X-42 下）。
#[derive(Debug, Clone, Copy)]
pub struct Refresh<'a> {
    /// 存着的 `refresh_token`。
    pub refresh_token: &'a str,
    /// 登录时的 `resource`（RFC 8707：续的时候带同一个）。
    pub resource: &'a str,
    /// 登录时给的范围，没有的不写。
    pub scope: Option<&'a str>,
}

/// 续期（`grant_type=refresh_token`，施工 X-42 下）：带同一个 `resource`、原来的 `scope`，有密钥的客户端照 `client_secret_post`。
/// 授权服务器没给新的 `refresh_token` 的，交回的那一格是空的，调的一方留旧的。
///
/// # Errors
///
/// 拒了的（`invalid_grant` 这类）[`Trouble::Refused`]；别的 [`Trouble::Unreachable`]。
pub async fn refresh(
    http: &Client,
    authorization: &Authorization,
    client: &Registered,
    asked: &Refresh<'_>,
) -> Result<Tokens, Trouble> {
    // 拼好就丢掉拼的那一个：它不能跨着 await 留着（不是 `Sync`）。
    let body = {
        let mut form = url::form_urlencoded::Serializer::new(String::new());
        form.append_pair("grant_type", "refresh_token")
            .append_pair("refresh_token", asked.refresh_token)
            .append_pair("client_id", &client.client_id)
            .append_pair("resource", asked.resource);
        if let Some(scope) = asked.scope {
            form.append_pair("scope", scope);
        }
        if let Some(secret) = &client.client_secret {
            form.append_pair("client_secret", secret);
        }
        form.finish()
    };
    tokens(http, authorization, body).await
}

/// 依次 `GET` 几个网址，第一个回 `200`、是 JSON 的交回；都不是的没有。连不上的当场停。
async fn first_json(http: &Client, urls: &[String]) -> Result<Option<Value>, Trouble> {
    for url in urls {
        let response = http
            .get(url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|error| Trouble::Unreachable(error.without_url().to_string()))?;
        if !response.status().is_success() {
            continue;
        }
        if let Ok(bytes) = response.bytes().await
            && let Ok(found) = serde_json::from_slice::<Value>(&bytes)
            && found.is_object()
        {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

/// 发一个请求，交回 JSON 的回答：`2xx` 的照收；别的照 OAuth 的错误回答读 `error`、`error_description`（说明截过）。
async fn send(request: miyu_http::RequestBuilder) -> Result<Value, Trouble> {
    let response = request
        .send()
        .await
        .map_err(|error| Trouble::Unreachable(error.without_url().to_string()))?;
    let ok = response.status().is_success();
    let status = response.status().as_u16();
    let bytes = response
        .bytes()
        .await
        .map_err(|error| Trouble::Unreachable(error.without_url().to_string()))?;
    let answer: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    if ok {
        return Ok(answer);
    }
    match answer["error"].as_str() {
        Some(error) => Err(Trouble::Refused {
            error: error.to_string(),
            description: answer["error_description"]
                .as_str()
                .map(|text| text.chars().take(DESCRIPTION_LIMIT).collect()),
        }),
        None => Err(Trouble::Unreachable(format!("HTTP {status}"))),
    }
}
