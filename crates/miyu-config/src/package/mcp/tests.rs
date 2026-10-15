//! `[[mcp]]`（施工 X-2）：每一格读成什么、没写的照什么；每一种写错报 `bad_mcp`、报在那一段；只带 MCP 服务的包是 `bundle`、
//! 哪种包能写；一个服务一个功能。

use super::super::{Code, PackageKind, read};
use super::*;

/// 只带 MCP 服务的包。
const BUNDLE: &str = r#"[package]
name = { en = "GitHub tools", zh = "GitHub 工具" }

[[mcp]]
name = "github"
command = ["npx", "-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_TOKEN = { secret = "github" }, LOG = "quiet", HOME_DIR = { env = "HOME" } }
venues = ["local", "group"]
timeout_ms = 120000
need_load = true

[[mcp]]
name = "fetch"
command = ["uvx", "mcp-server-fetch"]
"#;

#[test]
fn a_package_with_only_mcp_servers_is_a_bundle_and_reads_every_field() {
    let manifest = read(BUNDLE).expect("合写法");
    assert_eq!(manifest.kind, PackageKind::Bundle);
    assert_eq!(manifest.protocol, None, "bundle 不说协议");
    assert_eq!(manifest.mcp.len(), 2);
    let github = &manifest.mcp[0];
    assert_eq!(github.name, "github");
    assert_eq!(
        github.command,
        ["npx", "-y", "@modelcontextprotocol/server-github"]
    );
    assert_eq!(
        github.env,
        [
            (
                "GITHUB_TOKEN".to_string(),
                EnvValue::Reference(crate::secret::Reference::Secret("github".to_string()))
            ),
            ("LOG".to_string(), EnvValue::Text("quiet".to_string())),
            (
                "HOME_DIR".to_string(),
                EnvValue::Reference(crate::secret::Reference::Env("HOME".to_string()))
            ),
        ]
    );
    assert_eq!(github.venues, ["local", "group"]);
    assert_eq!(github.timeout_ms, 120_000);
    assert!(github.need_load, "施工 L-1");
    assert_eq!(github.line, Some(4));
    let fetch = &manifest.mcp[1];
    assert_eq!(fetch.env, []);
    assert_eq!(fetch.venues, ["local"], "没写的只给本机的会话");
    assert_eq!(fetch.timeout_ms, TIMEOUT_DEFAULT);
    assert!(!fetch.need_load, "没写的全量");
    assert_eq!(fetch.line, Some(12));
}

#[test]
fn each_server_is_a_feature_of_its_own() {
    let manifest = read(BUNDLE).expect("合写法");
    let features = manifest.features_of("gh");
    let ids: Vec<&str> = features.iter().map(|feature| feature.id.as_str()).collect();
    assert_eq!(ids, ["mcp-github", "mcp-fetch"], "bundle 自己不另算一个");
    assert_eq!(features[0].service.as_deref(), Some("github"));
    assert_eq!(
        features[0].name.get("en").map(String::as_str),
        Some("github")
    );
    assert_eq!(features[0].line, Some(4));
    // 扩展包没写 [features] 的照旧整个包算一个，接着每个服务一个。
    let bridge = "[package]\nprotocol = [1, 1]\nname = { en = \"B\" }\n\n[command]\nname = \"b\"\nprogram = \"miyu-b\"\nabout = { en = \"B\" }\n\n[process]\n\n[[mcp]]\nname = \"fetch\"\ncommand = [\"uvx\", \"mcp-server-fetch\"]\n";
    let manifest = read(bridge).expect("扩展包能带 MCP 服务");
    assert_eq!(manifest.kind, PackageKind::Process);
    let ids: Vec<String> = manifest
        .features_of("b")
        .into_iter()
        .map(|feature| feature.id)
        .collect();
    assert_eq!(ids, ["b", "mcp-fetch"]);
}

#[test]
fn only_extensions_and_bundles_carry_mcp_servers() {
    let mcp = "\n[[mcp]]\nname = \"fetch\"\ncommand = [\"uvx\", \"mcp-server-fetch\"]\n";
    for (head, kind) in [
        (
            "[package]\nprotocol = [1, 1]\nname = { en = \"U\" }\n\n[command]\nname = \"u\"\nprogram = \"miyu-u\"\nabout = { en = \"U\" }\n\n[ui]\n",
            "ui",
        ),
        (
            "[package]\nprotocol = [1, 1]\nname = { en = \"W\" }\n\n[worker]\nprogram = \"miyu-w\"\n",
            "worker",
        ),
        (
            "[package]\nprotocol = [1, 1]\nname = { en = \"B\" }\n\n[builtin]\n",
            "builtin",
        ),
    ] {
        let problem = read(&format!("{head}{mcp}")).expect_err(kind);
        assert_eq!(problem.code, Code::WrongKind, "{kind}：{problem}");
    }
    // 吉祥物加 MCP 服务的是 bundle，带着吉祥物。
    let both = format!("[package]\nname = {{ en = \"P\" }}\n\n[mascot]\nmodel = \"m.json\"\n{mcp}");
    let manifest = read(&both).expect("bundle 能带吉祥物");
    assert_eq!(manifest.kind, PackageKind::Bundle);
    assert!(manifest.mascot.is_some());
    // bundle 不说协议。
    let spoken = BUNDLE.replace("[package]\n", "[package]\nprotocol = [1, 1]\n");
    assert_eq!(read(&spoken).expect_err("写了协议").code, Code::WrongKind);
}

#[test]
fn every_mistake_is_bad_mcp_at_its_server() {
    let one = |body: &str| format!("[package]\nname = {{ en = \"P\" }}\n\n[[mcp]]\n{body}\n");
    for (body, detail) in [
        ("command = [\"x\"]", "mcp.name"),
        ("name = \"Git\"\ncommand = [\"x\"]", "mcp.name \"Git\""),
        ("name = \"a_b\"\ncommand = [\"x\"]", "mcp.name \"a_b\""),
        (
            "name = \"abcdefghijklmnopqrstuvwxyz1234567\"\ncommand = [\"x\"]",
            "mcp.name \"abcdefghijklmnopqrstuvwxyz1234567\"",
        ),
        ("name = \"g\"", "mcp.g.command"),
        ("name = \"g\"\ncommand = []", "mcp.g.command"),
        ("name = \"g\"\ncommand = [\"\"]", "mcp.g.command"),
        ("name = \"g\"\ncommand = \"npx\"", "mcp.g.command"),
        ("name = \"g\"\ncommand = [\"x\", 1]", "mcp.g.command"),
        (
            "name = \"g\"\ncommand = [\"x\"]\nenv = { A = 1 }",
            "mcp.g.env",
        ),
        (
            "name = \"g\"\ncommand = [\"x\"]\nenv = { \"A=B\" = \"c\" }",
            "mcp.g.env",
        ),
        (
            "name = \"g\"\ncommand = [\"x\"]\nenv = { A = { secret = \"Bad Name\" } }",
            "mcp.g.env",
        ),
        (
            "name = \"g\"\ncommand = [\"x\"]\nvenues = []",
            "mcp.g.venues",
        ),
        (
            "name = \"g\"\ncommand = [\"x\"]\nvenues = [\"room\"]",
            "mcp.g.venues",
        ),
        (
            "name = \"g\"\ncommand = [\"x\"]\nvenues = [\"local\", \"local\"]",
            "mcp.g.venues",
        ),
        (
            "name = \"g\"\ncommand = [\"x\"]\ntimeout_ms = 999",
            "mcp.g.timeout_ms",
        ),
        (
            "name = \"g\"\ncommand = [\"x\"]\ntimeout_ms = 3600001",
            "mcp.g.timeout_ms",
        ),
        (
            "name = \"g\"\ncommand = [\"x\"]\ntimeout_ms = \"60s\"",
            "mcp.g.timeout_ms",
        ),
        (
            "name = \"g\"\ncommand = [\"x\"]\nneed_load = \"yes\"",
            "mcp.g.need_load",
        ),
    ] {
        let problem = read(&one(body)).expect_err(body);
        assert_eq!(problem.code, Code::BadMcp, "{body}：{problem}");
        assert_eq!(problem.detail, detail, "{body}");
        assert_eq!(problem.line, Some(4), "报在那一段：{body}");
    }
    let unknown = read(&one("name = \"g\"\ncommand = [\"x\"]\nargs = []")).expect_err("多一格");
    assert_eq!(unknown.code, Code::UnknownKey);
    let twice = read(&format!(
        "{}\n[[mcp]]\nname = \"g\"\ncommand = [\"y\"]\n",
        one("name = \"g\"\ncommand = [\"x\"]")
    ))
    .expect_err("同名两段");
    assert_eq!((twice.code, twice.line), (Code::BadMcp, Some(8)), "{twice}");
    let table =
        read("[package]\nname = { en = \"P\" }\n\n[mcp]\nname = \"g\"\n").expect_err("写成一张表");
    assert_eq!(table.code, Code::NotATable);
}
