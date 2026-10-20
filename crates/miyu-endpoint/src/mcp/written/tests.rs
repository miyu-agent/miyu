//! 个人设置里一张 `[mcp.<名字>]` 的字（施工 X-26）：加在末尾、换在原处、删掉连同空行；别的字节一个不变；放不进去的不改。

use serde_json::json;

use super::*;

fn entry(name: &str, command: &[&str]) -> Entry {
    Entry {
        name: name.to_string(),
        command: command.iter().map(ToString::to_string).collect(),
        env: Vec::new(),
        venues: None,
        timeout_ms: None,
        need_load: None,
        clicall: None,
    }
}

#[test]
fn a_block_is_written_in_the_settings_shape() {
    let mut full = entry("github", &["npx", "-y", "server"]);
    full.env = vec![
        ("A".to_string(), json!("1")),
        ("TOKEN".to_string(), json!({"secret": "github"})),
    ];
    full.venues = Some(vec!["local".to_string(), "group".to_string()]);
    full.timeout_ms = Some(1000);
    full.need_load = Some(true);
    assert_eq!(
        block(&full).unwrap(),
        "[mcp.github]\ncommand = [\"npx\", \"-y\", \"server\"]\nenv = { A = \"1\", TOKEN = { secret = \"github\" } }\nvenues = [\"local\", \"group\"]\ntimeout_ms = 1000\nneed_load = true\n"
    );
    assert_eq!(
        block(&entry("a b", &["x"])).unwrap(),
        "[mcp.\"a b\"]\ncommand = [\"x\"]\n",
        "名字不能裸着写的带引号，写法对不对由读的一方查"
    );
    let mut null = entry("x", &["x"]);
    null.env = vec![("A".to_string(), json!(null))];
    assert!(block(&null).is_none());
}

#[test]
fn a_new_server_goes_to_the_end_and_the_rest_stays() {
    let text = "#:schema x\n[ui]\nlanguage = \"zh\" # 中文\n";
    let block = block(&entry("fetch", &["uvx", "mcp-server-fetch"])).unwrap();
    assert_eq!(has(text, "fetch"), Some(false));
    let put = put(text, "fetch", &block).unwrap();
    assert_eq!(put, format!("{text}\n{block}"));
    assert_eq!(has(&put, "fetch"), Some(true));
    assert_eq!(
        super::put("", "fetch", &block).unwrap(),
        block,
        "空的不空行"
    );
    let crlf = "[ui]\r\nlanguage = \"zh\"";
    assert_eq!(
        super::put(crlf, "fetch", &block).unwrap(),
        format!("{crlf}\r\n\r\n{}", block.replace('\n', "\r\n")),
        "换行照原来的"
    );
}

#[test]
fn a_written_server_is_replaced_in_place_with_its_subtables() {
    let text = "[ui]\nlanguage = \"zh\"\n\n# 抓网页\n[mcp.fetch]\ncommand = [\"old\"]\ntimeout_ms = 5000 # 慢\n\n[mcp.fetch.env]\nA = \"1\"\n\n[mcp.time]\ncommand = [\"time\"]\n";
    let block = block(&entry("fetch", &["new"])).unwrap();
    let put = put(text, "fetch", &block).unwrap();
    assert_eq!(
        put,
        "[ui]\nlanguage = \"zh\"\n\n# 抓网页\n[mcp.fetch]\ncommand = [\"new\"]\n\n[mcp.time]\ncommand = [\"time\"]\n"
    );
    let (servers, problems) = personal_mcp(&put);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(
        servers
            .iter()
            .map(|server| server.name.as_str())
            .collect::<Vec<_>>(),
        ["fetch", "time"],
        "先后不变"
    );
    assert!(servers[0].env.is_empty(), "整条换掉：子表也没了");
}

#[test]
fn other_shapes_are_cut_and_the_new_one_appended() {
    let dotted = "[mcp]\nfetch.command = [\"old\"]\nfetch.timeout_ms = 5000\ntime = { command = [\"time\"] }\n";
    let block = block(&entry("fetch", &["new"])).unwrap();
    assert_eq!(
        put(dotted, "fetch", &block).unwrap(),
        format!("[mcp]\ntime = {{ command = [\"time\"] }}\n\n{block}")
    );
    let inline = "[mcp]\ntime = { command = [\"time\"] }\n";
    let block = super::block(&entry("time", &["new"])).unwrap();
    assert_eq!(
        put(inline, "time", &block).unwrap(),
        format!("[mcp]\n\n{block}")
    );
}

#[test]
fn removing_takes_the_blank_lines_with_it() {
    let text = "[ui]\nlanguage = \"zh\"\n\n[mcp.fetch]\ncommand = [\"f\"]\n\n[mcp.time]\ncommand = [\"t\"]\n";
    assert_eq!(
        remove(text, "fetch").unwrap(),
        "[ui]\nlanguage = \"zh\"\n\n[mcp.time]\ncommand = [\"t\"]\n"
    );
    assert_eq!(
        remove(text, "time").unwrap(),
        "[ui]\nlanguage = \"zh\"\n\n[mcp.fetch]\ncommand = [\"f\"]\n",
        "在末尾的连前面的空行"
    );
    assert_eq!(remove(text, "nope").unwrap(), text, "没写的照原样");
    assert_eq!(
        remove("[mcp]\nfetch = { command = [\"f\"] }\n", "fetch").unwrap(),
        "[mcp]\n"
    );
}

#[test]
fn shapes_that_cannot_hold_a_server_are_refused() {
    let block = block(&entry("fetch", &["new"])).unwrap();
    for text in [
        "mcp = 3\n",
        "mcp = { fetch = { command = [\"f\"] } }\n",
        "[[mcp]]\nname = \"fetch\"\n",
        "not toml = = \n",
    ] {
        assert_eq!(has(text, "fetch"), None, "{text}");
        assert_eq!(put(text, "fetch", &block), None, "{text}");
        assert_eq!(remove(text, "fetch"), None, "{text}");
    }
    assert_eq!(
        put("", "fetch", "[mcp.fetch]\ncommand = []\n"),
        None,
        "写错的一张不写"
    );
}
