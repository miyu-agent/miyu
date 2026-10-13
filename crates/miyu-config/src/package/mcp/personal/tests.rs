//! 个人设置里的 `[mcp.<名字>]`（施工 F-9 上）：格子同清单、名字是表名；写错的那一个不收、报在它那一行，别的照收；没有这张表、
//! 整份不是 TOML 的什么都没有。

use super::super::super::Code;
use super::super::EnvValue;
use super::*;
use crate::secret::Reference;

/// 两个服务，夹着别的设置。
const SETTINGS: &str = r#"#:schema ../../state/config/settings.schema.json

[ui]
language = "zh"

[mcp.github]
command = ["npx", "-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_TOKEN = { secret = "github" }, LOG = "quiet" }
venues = ["local", "private"]
timeout_ms = 120000

[mcp.fetch]
command = ["uvx", "mcp-server-fetch"]
"#;

#[test]
fn every_table_under_mcp_is_one_server_named_by_its_key() {
    let (servers, problems) = personal(SETTINGS);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(servers.len(), 2);
    let github = &servers[0];
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
                EnvValue::Reference(Reference::Secret("github".to_string()))
            ),
            ("LOG".to_string(), EnvValue::Text("quiet".to_string())),
        ]
    );
    assert_eq!(github.venues, ["local", "private"]);
    assert_eq!(github.timeout_ms, 120_000);
    assert_eq!(github.line, Some(6));
    let fetch = &servers[1];
    assert_eq!(fetch.name, "fetch");
    assert_eq!(fetch.venues, ["local"], "没写的只给本机的会话");
    assert_eq!(fetch.timeout_ms, super::super::TIMEOUT_DEFAULT);
    assert_eq!(fetch.line, Some(12));
}

#[test]
fn the_dotted_and_inline_ways_of_writing_read_the_same() {
    let text = "mcp.fetch.command = [\"uvx\", \"mcp-server-fetch\"]\nmcp.time = { command = [\"uvx\", \"mcp-server-time\"] }\n";
    let (servers, problems) = personal(text);
    assert!(problems.is_empty(), "{problems:?}");
    let names: Vec<&str> = servers.iter().map(|server| server.name.as_str()).collect();
    assert_eq!(names, ["fetch", "time"]);
    assert_eq!(servers[1].line, Some(2));
}

#[test]
fn a_wrong_server_is_left_out_and_reported_on_its_line() {
    let text = r#"[mcp.good]
command = ["uvx", "mcp-server-fetch"]

[mcp.Bad]
command = ["x"]

[mcp.empty]
command = []

[mcp.extra]
command = ["x"]
name = "extra"

[mcp.slow]
command = ["x"]
timeout_ms = 10
"#;
    let (servers, problems) = personal(text);
    let names: Vec<&str> = servers.iter().map(|server| server.name.as_str()).collect();
    assert_eq!(names, ["good"], "写错的那几个不收，别的照收");
    let found: Vec<(Option<usize>, Code, &str)> = problems
        .iter()
        .map(|problem| (problem.line, problem.code, problem.detail.as_str()))
        .collect();
    assert_eq!(
        found,
        [
            (Some(4), Code::BadMcp, "mcp \"Bad\""),
            (Some(7), Code::BadMcp, "mcp.empty.command"),
            (Some(12), Code::UnknownKey, "mcp.extra.name"),
            (Some(14), Code::BadMcp, "mcp.slow.timeout_ms"),
        ]
    );
}

#[test]
fn mcp_written_as_a_value_is_one_problem() {
    let (servers, problems) = personal("mcp = \"github\"\n");
    assert!(servers.is_empty());
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].code, Code::BadMcp);
    assert_eq!(problems[0].line, Some(1));
    let (servers, problems) = personal("[mcp]\ngithub = \"npx\"\n");
    assert!(servers.is_empty());
    assert_eq!(problems[0].detail, "mcp.github");
}

#[test]
fn nothing_to_read_is_nothing() {
    assert_eq!(
        personal("[ui]\nlanguage = \"zh\"\n"),
        (Vec::new(), Vec::new())
    );
    assert_eq!(personal(""), (Vec::new(), Vec::new()));
    assert_eq!(
        personal("[mcp.x\n"),
        (Vec::new(), Vec::new()),
        "整份读不成的由配置那一边报"
    );
    let bom = format!("\u{FEFF}{SETTINGS}");
    assert_eq!(personal(&bom).0.len(), 2, "开头的 BOM 不碍事");
}
