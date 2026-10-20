//! 个人设置里的服务（施工 F-9 下）：只差在第几行的算一样（加一行注释不重起）；命令第一项的 `~/` 照家目录换。

use std::path::Path;

use super::*;

fn server(name: &str, command: &[&str], line: usize) -> McpServer {
    McpServer {
        name: name.to_string(),
        command: command.iter().map(ToString::to_string).collect(),
        env: Vec::new(),
        venues: vec!["local".to_string()],
        timeout_ms: 600_000,
        need_load: false,
        clicall: true,
        line: Some(line),
    }
}

#[test]
fn only_the_line_may_differ() {
    let a = server("fetch", &["uvx", "mcp-server-fetch"], 4);
    assert!(same(&a, &server("fetch", &["uvx", "mcp-server-fetch"], 9)));
    assert!(!same(&a, &server("fetch", &["uvx", "other"], 4)));
    assert!(!same(
        &a,
        &McpServer {
            timeout_ms: 1_000,
            ..a.clone()
        }
    ));
    let env = vec![(
        "TOKEN".to_string(),
        miyu_config::package::EnvValue::Text("x".to_string()),
    )];
    assert!(
        !same(&a, &McpServer { env, ..a.clone() }),
        "环境变量变了算变了"
    );
    assert!(
        !same(
            &a,
            &McpServer {
                clicall: !a.clicall,
                ..a.clone()
            }
        ),
        "只改脚本里能不能调也重起（施工 M-2）"
    );
    assert!(!same(
        &a,
        &McpServer {
            venues: vec!["group".to_string()],
            ..a.clone()
        }
    ));
    assert!(same_list(
        std::slice::from_ref(&a),
        &[server("fetch", &["uvx", "mcp-server-fetch"], 1)]
    ));
    assert!(!same_list(std::slice::from_ref(&a), &[]));
    let b = server("time", &["uvx", "mcp-server-time"], 8);
    assert!(
        !same_list(&[a.clone(), b.clone()], &[b, a]),
        "先后变了也算变了"
    );
}

#[test]
fn a_tilde_command_follows_the_home() {
    let home = if cfg!(windows) {
        Path::new(r"C:\Users\alice")
    } else {
        Path::new("/home/alice")
    };
    let changed = tilde(&server("own", &["~/bin/server", "--flag"], 1), home);
    assert_eq!(
        changed.command,
        [
            home.join("bin/server").to_string_lossy().into_owned(),
            "--flag".to_string()
        ]
    );
    let plain = tilde(&server("own", &["npx", "~/x"], 1), home);
    assert_eq!(plain.command, ["npx", "~/x"], "只换第一项");
    let unknown = tilde(&server("own", &["~/bin/server"], 1), Path::new(""));
    assert_eq!(unknown.command, ["~/bin/server"], "不知道家目录的照原样");
}
