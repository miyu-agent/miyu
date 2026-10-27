//! 要不要登录（施工 X-42 上）：网址的服务、这张表没写令牌、回过 `Bearer` 的 `401` 才要；存进密钥的那一份读写一样。

use miyu_config::package::{EnvValue, McpServer, Reach};

use super::*;

fn url_server(bearer: Option<&str>, headers: &[(&str, &str)]) -> McpServer {
    McpServer {
        name: "notes".to_string(),
        reach: Reach::Url {
            url: "https://mcp.example.com/mcp".to_string(),
            bearer: bearer.map(|token| EnvValue::Text(token.to_string())),
            headers: headers
                .iter()
                .map(|(name, value)| ((*name).to_string(), EnvValue::Text((*value).to_string())))
                .collect(),
        },
        venues: vec!["local".to_string()],
        timeout_ms: 600_000,
        clicall: true,
        line: None,
    }
}

const CHALLENGE: &str = r#"Bearer resource_metadata="https://mcp.example.com/.well-known/oauth-protected-resource/mcp""#;

#[test]
fn only_a_url_server_without_its_own_token_needs_a_login() {
    let plain = url_server(None, &[]);
    assert_eq!(
        needed(&plain, Some(CHALLENGE.to_string())).and_then(|found| found.resource_metadata),
        Some("https://mcp.example.com/.well-known/oauth-protected-resource/mcp".to_string())
    );
    assert_eq!(needed(&plain, None), None, "没回过 401 的不要");
    assert_eq!(
        needed(&plain, Some(r#"Basic realm="x""#.to_string())),
        None,
        "不是 Bearer 的不要"
    );
    assert_eq!(
        needed(&url_server(Some("t"), &[]), Some(CHALLENGE.to_string())),
        None,
        "写了 bearer 的是人给的令牌"
    );
    assert_eq!(
        needed(
            &url_server(None, &[("authorization", "Token x")]),
            Some(CHALLENGE.to_string())
        ),
        None,
        "headers 里写了 Authorization（不分大小写）的也是"
    );
    let process = McpServer {
        reach: Reach::Command {
            command: vec!["npx".to_string()],
            env: Vec::new(),
        },
        ..url_server(None, &[])
    };
    assert_eq!(
        needed(&process, Some(CHALLENGE.to_string())),
        None,
        "起进程的不走登录"
    );
}

#[test]
fn the_stored_login_reads_back_the_same() {
    let stored = Stored {
        access_token: "a".to_string(),
        refresh_token: Some("r".to_string()),
        expires_at: Some(1_800_000_000),
        scope: Some("files:read".to_string()),
        issuer: "https://auth.example.com".to_string(),
        client_id: "c".to_string(),
        client_secret: None,
        port: 33418,
        registered: true,
        resource: Some("https://api.example.com/mcp".to_string()),
    };
    let text = serde_json::to_string(&stored).expect("写得成");
    assert!(!text.contains('\n'), "一行，进得了密钥文件");
    assert!(!text.contains("client_secret"), "没有的格不写");
    assert_eq!(
        serde_json::from_str::<Stored>(&text).expect("读得回"),
        stored
    );
    assert_eq!(secret_name("notes"), "mcp-oauth-notes");
    assert!(miyu_config::secret::valid_name(&secret_name("notes")));
}
