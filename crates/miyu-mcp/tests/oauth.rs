//! 网址的服务登录（施工 X-42 上）：假的要登录的服务（`HttpServer::protect`）、假授权服务器（`AuthServer`），测试自己当浏览器。
//! 没令牌回 `401`、照挑战找到授权服务器；动态注册、PKCE、`state`、`iss`、`resource` 一路对上，换来的令牌服务认；只照两个
//! well-known 也找得到；要精确端口的授权服务器同一个端口第二次照过、换了端口不跳回来；`code_verifier` 不对的换不到令牌。

use miyu_http::Proxy;
use miyu_mcp::oauth::{
    Asked, Callback, Challenge, Exchange, Pkce, Registered, Trouble, authorize_url, canonical,
    challenge, discover, exchange, iss_ok, listen, random_state, register,
};
use miyu_mcp::testkit::http::HttpServer;
use miyu_mcp::testkit::oauth::{AuthServer, browse};

fn http() -> miyu_http::Client {
    miyu_http::client(Proxy::Off).expect("客户端")
}

/// 没带令牌发一条，交回 `401` 的挑战。
async fn challenged(server: &HttpServer) -> Challenge {
    let response = http()
        .post(&server.url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .body(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#)
        .send()
        .await
        .expect("发得出去");
    assert_eq!(response.status().as_u16(), 401);
    let header = response
        .headers()
        .get("www-authenticate")
        .and_then(|value| value.to_str().ok())
        .expect("有挑战")
        .to_string();
    challenge(&header).expect("是 Bearer")
}

/// 走一遍授权页：交回回调里的东西和回调的地址。
async fn authorized(
    found: &miyu_mcp::oauth::Discovered,
    client: &Registered,
    port: Option<u16>,
    pkce: &Pkce,
    state: &str,
) -> (Option<Callback>, String, u16) {
    let listener = listen(port).await.expect("听得了");
    let redirect = listener.redirect_uri();
    let url = authorize_url(
        &found.authorization,
        &Asked {
            client_id: &client.client_id,
            redirect_uri: &redirect,
            scope: found.scope.as_deref(),
            resource: &found.resource,
            state,
            challenge: &pkce.challenge,
        },
    );
    let browsing = tokio::spawn(async move { browse(&url).await });
    let waited = tokio::select! {
        got = listener.next() => {
            let (got, reply) = got.expect("等得到");
            reply.send(true, "done").await;
            Some(got)
        }
        status = async { browsing.await.expect("浏览器") } => {
            assert_ne!(status.0, 200, "没跳回来的是授权服务器拒了");
            None
        }
    };
    (waited, redirect, listener.port())
}

#[tokio::test]
async fn a_protected_server_is_logged_into_and_the_token_works() {
    let auth = AuthServer::start().await;
    let server = HttpServer::start(false).await;
    server.protect(&auth.issuer, "access-1");
    let challenge = challenged(&server).await;
    assert_eq!(challenge.scope.as_deref(), Some("files:read"));
    let found = discover(&http(), &server.url, &challenge)
        .await
        .expect("找得到");
    assert_eq!(Some(found.resource.clone()), canonical(&server.url));
    assert_eq!(found.scope.as_deref(), Some("files:read offline_access"));
    let listener = listen(None).await.expect("听得了");
    let client = register(&http(), &found.authorization, &listener.redirect_uri())
        .await
        .expect("注册得了");
    let port = listener.port();
    drop(listener);
    let pkce = Pkce::new().expect("有随机数");
    let state = random_state().expect("有随机数");
    let (got, redirect, _) = authorized(&found, &client, Some(port), &pkce, &state).await;
    let Some(Callback::Code {
        code,
        state: back,
        iss,
    }) = got
    else {
        panic!("跳回来了：{got:?}");
    };
    assert_eq!(back, state);
    assert!(iss_ok(
        found.authorization.iss_supported,
        iss.as_deref(),
        &found.authorization.issuer
    ));
    let tokens = exchange(
        &http(),
        &found.authorization,
        &client,
        &Exchange {
            code: &code,
            verifier: &pkce.verifier,
            redirect_uri: &redirect,
            resource: &found.resource,
        },
    )
    .await
    .expect("换得到");
    assert_eq!(tokens.access_token, "access-1");
    assert_eq!(tokens.refresh_token.as_deref(), Some("refresh-1"));
    assert_eq!(tokens.expires_in, Some(3600));
    let served = http()
        .post(&server.url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header("Authorization", format!("Bearer {}", tokens.access_token))
        .body(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#)
        .send()
        .await
        .expect("发得出去");
    assert_eq!(served.status().as_u16(), 200, "换来的令牌服务认");
}

#[tokio::test]
async fn the_well_known_paths_are_enough_without_a_header() {
    let auth = AuthServer::start().await;
    let server = HttpServer::start(false).await;
    server.protect(&auth.issuer, "access-1");
    let found = discover(&http(), &server.url, &Challenge::default())
        .await
        .expect("照 well-known 找得到");
    assert_eq!(found.authorization.issuer, auth.issuer);
    let asked: Vec<String> = server
        .received()
        .into_iter()
        .map(|request| request.path)
        .collect();
    assert_eq!(
        asked,
        ["/.well-known/oauth-protected-resource/mcp"],
        "先照服务的路径插进去的那一个"
    );
}

#[tokio::test]
async fn an_exact_port_server_takes_the_registered_port_only() {
    let auth = AuthServer::exact_port().await;
    let server = HttpServer::start(false).await;
    server.protect(&auth.issuer, "access-1");
    let found = discover(&http(), &server.url, &challenged(&server).await)
        .await
        .expect("找得到");
    let listener = listen(None).await.expect("听得了");
    let port = listener.port();
    let client = register(&http(), &found.authorization, &listener.redirect_uri())
        .await
        .expect("注册得了");
    drop(listener);
    let pkce = Pkce::new().expect("有随机数");
    let (same, _, _) = authorized(&found, &client, Some(port), &pkce, "s1").await;
    assert!(
        matches!(same, Some(Callback::Code { .. })),
        "同一个端口照过：{same:?}"
    );
    let (moved, _, moved_port) = authorized(&found, &client, None, &pkce, "s2").await;
    assert_ne!(moved_port, port);
    assert_eq!(moved, None, "换了端口的不跳回来");
    let again = listen(Some(moved_port)).await.expect("听得了");
    let fresh = register(&http(), &found.authorization, &again.redirect_uri())
        .await
        .expect("照新端口重新注册");
    drop(again);
    let (after, _, _) = authorized(&found, &fresh, Some(moved_port), &pkce, "s3").await;
    assert!(
        matches!(after, Some(Callback::Code { .. })),
        "重新注册以后照过：{after:?}"
    );
    assert_eq!(auth.registrations(), 2);
}

#[tokio::test]
async fn a_wrong_verifier_gets_no_token() {
    let auth = AuthServer::start().await;
    let server = HttpServer::start(false).await;
    server.protect(&auth.issuer, "access-1");
    let found = discover(&http(), &server.url, &challenged(&server).await)
        .await
        .expect("找得到");
    let listener = listen(None).await.expect("听得了");
    let port = listener.port();
    let client = register(&http(), &found.authorization, &listener.redirect_uri())
        .await
        .expect("注册得了");
    drop(listener);
    let pkce = Pkce::new().expect("有随机数");
    let (got, redirect, _) = authorized(&found, &client, Some(port), &pkce, "s1").await;
    let Some(Callback::Code { code, .. }) = got else {
        panic!("跳回来了：{got:?}");
    };
    let wrong = Pkce::new().expect("有随机数");
    let refused = exchange(
        &http(),
        &found.authorization,
        &client,
        &Exchange {
            code: &code,
            verifier: &wrong.verifier,
            redirect_uri: &redirect,
            resource: &found.resource,
        },
    )
    .await;
    assert_eq!(
        refused,
        Err(Trouble::Refused {
            error: "invalid_grant".to_string(),
            description: None
        })
    );
}
