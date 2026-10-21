use serde_json::json;

use miyu_config::secret::Reference;

use super::*;

fn server(command: &[&str], env: Vec<(String, EnvValue)>) -> McpServer {
    McpServer {
        name: "web".to_string(),
        reach: Reach::Command {
            command: command.iter().map(|part| (*part).to_string()).collect(),
            env,
        },
        venues: vec!["local".to_string()],
        timeout_ms: 600_000,
        need_load: false,
        clicall: true,
        line: None,
    }
}

fn written(dir: &Path, server: &McpServer, tools: Vec<Value>) -> PathBuf {
    let (command, env) = key(server);
    let cached = Cached {
        command,
        env,
        era: Era::Legacy("2025-11-25".to_string()),
        tools,
        instructions: Some("Use fetch for pages.".to_string()),
    };
    let path = dir.join("web.json");
    std::fs::write(&path, serde_json::to_vec(&cached).unwrap()).unwrap();
    path
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("miyu-mcp-cache-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 命令、环境变量的写法都和记着的一样才用：交回时代和认得出的工具，认不出的那一件不要。
#[test]
fn a_cache_is_used_only_while_the_command_and_env_are_the_same() {
    let dir = scratch("same");
    let env = vec![
        ("A".to_string(), EnvValue::Text("1".to_string())),
        (
            "TOKEN".to_string(),
            EnvValue::Reference(Reference::Secret("github".to_string())),
        ),
    ];
    let old = server(&["npx", "server"], env.clone());
    let path = written(
        &dir,
        &old,
        vec![
            json!({"name": "fetch", "inputSchema": {"type": "object"}}),
            json!({"name": "broken"}),
        ],
    );
    let Remembered {
        era,
        tools,
        instructions,
    } = read_at(&path, &old).expect("一样的用");
    assert_eq!(era, Era::Legacy("2025-11-25".to_string()));
    assert_eq!(
        instructions.as_deref(),
        Some("Use fetch for pages."),
        "说明跟着记（施工 X-39）"
    );
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>(),
        ["fetch"]
    );
    assert!(
        read_at(&path, &server(&["npx", "other"], env.clone())).is_none(),
        "命令变了不用"
    );
    let mut changed = env;
    changed[1].1 = EnvValue::Reference(Reference::Env("GITHUB_TOKEN".to_string()));
    assert!(
        read_at(&path, &server(&["npx", "server"], changed)).is_none(),
        "环境变量的写法变了不用"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// 没有的、写坏了的都不用；引用照写法记，不记取出来的值。
#[test]
fn a_missing_or_broken_cache_is_not_used_and_secrets_are_not_kept() {
    let dir = scratch("broken");
    let web = server(&["npx"], Vec::new());
    assert!(read_at(&dir.join("web.json"), &web).is_none());
    std::fs::write(dir.join("web.json"), b"{not json").unwrap();
    assert!(read_at(&dir.join("web.json"), &web).is_none());
    let secret = server(
        &["npx"],
        vec![(
            "TOKEN".to_string(),
            EnvValue::Reference(Reference::Secret("github".to_string())),
        )],
    );
    assert_eq!(key(&secret).1, [r#"TOKEN={ secret = "github" }"#]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_url_server_is_keyed_by_its_url_and_header_names_not_values() {
    let remote = |token: &str| McpServer {
        reach: Reach::Url {
            url: "https://mcp.example.com/mcp".to_string(),
            bearer: Some(EnvValue::Text(token.to_string())),
            headers: vec![("X-Team".to_string(), EnvValue::Text("core".to_string()))],
        },
        ..server(&["x"], Vec::new())
    };
    assert_eq!(
        key(&remote("a")),
        (
            vec!["https://mcp.example.com/mcp".to_string()],
            vec!["Authorization".to_string(), "X-Team".to_string()]
        )
    );
    assert_eq!(
        key(&remote("a")),
        key(&remote("b")),
        "换了令牌，目录照旧能用"
    );
}

/// 以前的缓存没有说明那一格（施工 X-39 以前写的）：照样认得，说明是空的。
#[test]
fn an_older_cache_without_instructions_still_reads() {
    let dir = scratch("older");
    let old = server(&["npx", "server"], Vec::new());
    let (command, env) = key(&old);
    let path = dir.join("web.json");
    let raw = json!({
        "command": command,
        "env": env,
        "era": {"era": "legacy", "version": "2025-11-25"},
        "tools": [{"name": "fetch", "inputSchema": {"type": "object"}}],
    });
    std::fs::write(&path, raw.to_string()).unwrap();
    let remembered = read_at(&path, &old).expect("认得");
    assert_eq!(remembered.instructions, None);
    assert_eq!(remembered.tools.len(), 1);
}
