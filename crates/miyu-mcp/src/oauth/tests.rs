//! 登录的纯逻辑（施工 X-42 上）：照 MCP 规范 2026-07-28 的 Authorization 和它引的几份 RFC 里的例子。

use serde_json::json;

use super::*;

#[test]
fn the_bearer_challenge_gives_where_the_metadata_is_and_the_scope() {
    assert_eq!(
        challenge(
            r#"Bearer resource_metadata="https://mcp.example.com/.well-known/oauth-protected-resource", scope="files:read""#
        ),
        Some(Challenge {
            resource_metadata: Some(
                "https://mcp.example.com/.well-known/oauth-protected-resource".to_string()
            ),
            scope: Some("files:read".to_string()),
            error: None,
        })
    );
    assert_eq!(
        challenge(r#"bearer error="insufficient_scope", scope="files:write""#)
            .and_then(|challenge| challenge.error),
        Some("insufficient_scope".to_string()),
        "方案名不分大小写"
    );
    assert_eq!(
        challenge(r#"Basic realm="x", Bearer scope="a b""#).and_then(|challenge| challenge.scope),
        Some("a b".to_string()),
        "几种方案并在一起的认 Bearer 那一段"
    );
    assert_eq!(challenge("Bearer"), Some(Challenge::default()));
    assert_eq!(challenge(r#"Basic realm="x""#), None, "没有 Bearer 的不认");
}

#[test]
fn the_canonical_uri_is_lowercase_without_fragment_or_trailing_slash() {
    assert_eq!(
        canonical("HTTPS://MCP.Example.com/mcp/").as_deref(),
        Some("https://mcp.example.com/mcp")
    );
    assert_eq!(
        canonical("https://mcp.example.com/").as_deref(),
        Some("https://mcp.example.com")
    );
    assert_eq!(
        canonical("https://mcp.example.com:8443/server/mcp#part").as_deref(),
        Some("https://mcp.example.com:8443/server/mcp")
    );
    assert_eq!(canonical("mcp.example.com"), None, "没有 scheme 的不是网址");
}

#[test]
fn metadata_is_looked_for_in_the_spec_order() {
    assert_eq!(
        resource_metadata_urls("https://example.com/public/mcp"),
        [
            "https://example.com/.well-known/oauth-protected-resource/public/mcp",
            "https://example.com/.well-known/oauth-protected-resource",
        ]
    );
    assert_eq!(
        resource_metadata_urls("https://example.com"),
        ["https://example.com/.well-known/oauth-protected-resource"],
        "服务在根上的只有一个"
    );
    assert_eq!(
        authorization_metadata_urls("https://auth.example.com/tenant1"),
        [
            "https://auth.example.com/.well-known/oauth-authorization-server/tenant1",
            "https://auth.example.com/.well-known/openid-configuration/tenant1",
            "https://auth.example.com/tenant1/.well-known/openid-configuration",
        ]
    );
    assert_eq!(
        authorization_metadata_urls("https://auth.example.com"),
        [
            "https://auth.example.com/.well-known/oauth-authorization-server",
            "https://auth.example.com/.well-known/openid-configuration",
        ]
    );
}

fn metadata() -> serde_json::Value {
    json!({
        "issuer": "https://auth.example.com",
        "authorization_endpoint": "https://auth.example.com/authorize",
        "token_endpoint": "https://auth.example.com/token",
        "registration_endpoint": "https://auth.example.com/register",
        "scopes_supported": ["files:read", "offline_access"],
        "code_challenge_methods_supported": ["plain", "S256"],
        "authorization_response_iss_parameter_supported": true,
    })
}

#[test]
fn authorization_metadata_must_name_the_same_issuer_and_support_s256() {
    let read = authorization(&metadata(), "https://auth.example.com").expect("认得");
    assert_eq!(
        read.authorization_endpoint,
        "https://auth.example.com/authorize"
    );
    assert_eq!(read.token_endpoint, "https://auth.example.com/token");
    assert_eq!(
        read.registration_endpoint.as_deref(),
        Some("https://auth.example.com/register")
    );
    assert!(read.iss_supported);
    assert_eq!(
        authorization(&metadata(), "https://attacker.example"),
        Err(Problem::Issuer),
        "issuer 和拼网址用的不一样的不用"
    );
    let mut plain = metadata();
    plain["code_challenge_methods_supported"] = json!(["plain"]);
    assert_eq!(
        authorization(&plain, "https://auth.example.com"),
        Err(Problem::NoS256)
    );
    let mut silent = metadata();
    silent
        .as_object_mut()
        .expect("是表")
        .remove("code_challenge_methods_supported");
    assert_eq!(
        authorization(&silent, "https://auth.example.com"),
        Err(Problem::NoS256),
        "没写支持哪几种的当不支持，不降级"
    );
    let mut insecure = metadata();
    insecure["token_endpoint"] = json!("http://auth.example.com/token");
    assert_eq!(
        authorization(&insecure, "https://auth.example.com"),
        Err(Problem::Insecure),
        "只认 https（本机的测试例外）"
    );
}

#[test]
fn the_s256_challenge_follows_rfc_7636() {
    assert_eq!(
        challenge_of("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
    let made = Pkce::new().expect("有随机数");
    assert_eq!(made.verifier.len(), 43, "32 个字节的 base64url");
    assert_eq!(made.challenge, challenge_of(&made.verifier));
    assert_ne!(Pkce::new().expect("有随机数").verifier, made.verifier);
    assert_eq!(random_state().expect("有随机数").len(), 43);
}

#[test]
fn the_authorization_url_carries_pkce_state_resource_and_scope() {
    let read = authorization(&metadata(), "https://auth.example.com").expect("认得");
    let url = authorize_url(
        &read,
        &Asked {
            client_id: "abc",
            redirect_uri: "http://127.0.0.1:33418/callback",
            scope: Some("files:read offline_access"),
            resource: "https://mcp.example.com/mcp",
            state: "s1",
            challenge: "c1",
        },
    );
    let parsed = url::Url::parse(&url).expect("是网址");
    assert_eq!(parsed.path(), "/authorize");
    let pairs: Vec<(String, String)> = parsed.query_pairs().into_owned().collect();
    let pair = |key: &str, value: &str| (key.to_string(), value.to_string());
    assert_eq!(
        pairs,
        [
            pair("response_type", "code"),
            pair("client_id", "abc"),
            pair("redirect_uri", "http://127.0.0.1:33418/callback"),
            pair("code_challenge", "c1"),
            pair("code_challenge_method", "S256"),
            pair("state", "s1"),
            pair("resource", "https://mcp.example.com/mcp"),
            pair("scope", "files:read offline_access"),
        ]
    );
    let none = authorize_url(
        &read,
        &Asked {
            client_id: "abc",
            redirect_uri: "http://127.0.0.1:1/callback",
            scope: None,
            resource: "https://mcp.example.com",
            state: "s",
            challenge: "c",
        },
    );
    assert!(!none.contains("scope="), "没有要的范围不写：{none}");
}

#[test]
fn the_callback_gives_the_code_or_the_error() {
    assert_eq!(
        callback("/callback?code=abc&state=s1&iss=https%3A%2F%2Fauth.example.com"),
        Some(Callback::Code {
            code: "abc".to_string(),
            state: "s1".to_string(),
            iss: Some("https://auth.example.com".to_string()),
        })
    );
    assert_eq!(
        callback("/callback?error=access_denied&error_description=No+thanks&state=s1"),
        Some(Callback::Error {
            error: "access_denied".to_string(),
            description: Some("No thanks".to_string()),
            state: Some("s1".to_string()),
            iss: None,
        })
    );
    assert_eq!(callback("/favicon.ico"), None, "别的路径不是回调");
    assert_eq!(callback("/callback?code=abc"), None, "没有 state 的不认");
}

#[test]
fn the_issuer_in_the_answer_follows_the_rfc_9207_table() {
    let recorded = "https://auth.example.com";
    assert!(iss_ok(true, Some(recorded), recorded));
    assert!(!iss_ok(true, Some("https://evil.example"), recorded));
    assert!(!iss_ok(true, None, recorded), "说了会带、没带的拒");
    assert!(iss_ok(false, Some(recorded), recorded));
    assert!(
        !iss_ok(false, Some("https://evil.example"), recorded),
        "带了就比"
    );
    assert!(iss_ok(false, None, recorded));
    assert!(
        !iss_ok(true, Some("https://auth.example.com/"), recorded),
        "照原样比，不去末尾的 /"
    );
}

#[test]
fn the_scope_follows_the_challenge_then_the_resource_metadata() {
    let strings = |items: &[&str]| {
        items
            .iter()
            .map(|item| (*item).to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        scope(
            Some("files:read"),
            &strings(&["a", "b"]),
            &strings(&["offline_access"])
        )
        .as_deref(),
        Some("files:read offline_access"),
        "挑战里写了的照它，授权服务器认 offline_access 的加上"
    );
    assert_eq!(
        scope(None, &strings(&["a", "b"]), &[]).as_deref(),
        Some("a b"),
        "没写的照受保护资源元数据的 scopes_supported"
    );
    assert_eq!(
        scope(None, &[], &strings(&["offline_access"])),
        None,
        "都没有的不写：只写 offline_access 会丢了默认的范围"
    );
    assert_eq!(
        scope(Some("offline_access x"), &[], &strings(&["offline_access"])).as_deref(),
        Some("offline_access x"),
        "已经有的不重复加"
    );
}
