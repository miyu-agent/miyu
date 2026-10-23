//! `miyu call` 的写法（设计 39 第六节第 1、2 条）。

use super::*;

fn args(text: &[&str]) -> Vec<String> {
    text.iter().map(|arg| (*arg).to_string()).collect()
}

fn one(text: &[&str]) -> Value {
    let mut requests = parse(&args(text)).expect("写法对");
    assert_eq!(requests.len(), 1);
    requests.remove(0)
}

fn call_args(request: &Value) -> Value {
    serde_json::from_str(request["args"].as_str().unwrap()).unwrap()
}

#[test]
fn named_params_are_strings_and_repeats_are_arrays() {
    let request = one(&[
        "web_search",
        "--query",
        "niri 输入法",
        "--limit=8",
        "--site",
        "a",
        "--site",
        "b",
    ]);
    assert_eq!(request["op"], "call");
    assert_eq!(request["tool"], "web_search");
    assert_eq!(request["v"], VERSION);
    assert_eq!(
        call_args(&request),
        serde_json::json!({"query": "niri 输入法", "limit": "8", "site": ["a", "b"]}),
        "值一律是字符串，核心照工具的格式修"
    );
}

#[test]
fn json_args_take_an_object_and_named_ones_win() {
    let request = one(&[
        "kb_read",
        "--json",
        r#"{"kb":"notes","path":"a.md","page":2}"#,
        "--page",
        "3",
    ]);
    assert_eq!(
        call_args(&request),
        serde_json::json!({"kb": "notes", "path": "a.md", "page": "3"})
    );
    assert_eq!(
        parse(&args(&["kb_read", "--json", "[1]"])).unwrap_err(),
        BAD_JSON
    );
}

#[test]
fn schema_takes_one_or_several_names_either_way() {
    let requests = parse(&args(&["--schema", "web_search", "kb_read"])).unwrap();
    let tools: Vec<&str> = requests
        .iter()
        .map(|request| request["tool"].as_str().unwrap())
        .collect();
    assert_eq!(tools, ["web_search", "kb_read"]);
    assert!(requests.iter().all(|request| request["op"] == "schema"));
    assert_eq!(one(&["web_search", "--schema"])["op"], "schema");
}

#[test]
fn bad_usage_says_how_to_write_it() {
    let bad: [&[&str]; 5] = [
        &[],
        &["--schema"],
        &["--query", "x"],
        &["web_search", "query"],
        &["web_search", "--query"],
    ];
    for bad in bad {
        assert_eq!(parse(&args(bad)).unwrap_err(), HOW, "{bad:?}");
    }
}

#[test]
fn outside_a_command_miyu_runs_it_exits_two() {
    // 测试进程自己没有这两个变量：照「只能在 Miyu 跑的命令里用」说，不连核心。
    if std::env::var_os("MIYU_CALL_SOCK").is_some() {
        return;
    }
    let request = one(&["web_search", "--query", "x"]);
    assert_eq!(send(&request).unwrap_err(), (USAGE, OUTSIDE.to_string()));
}

/// Windows PowerShell 5.1 吃掉了双引号（施工 M-8）：直接写在命令行上的、有花括号没双引号的，报错接一句改用标准输入；别的读
/// 不懂的照旧只说那一句。
#[test]
fn json_that_lost_its_quotes_says_to_pipe_it() {
    let lost = parse(&args(&["web_search", "--json", "{query:niri}"])).unwrap_err();
    assert!(lost.starts_with(BAD_JSON.trim_end()), "{lost}");
    assert!(lost.ends_with(QUOTES), "{lost}");
    assert!(lost.contains("--json -"), "{lost}");
    assert_eq!(
        parse(&args(&["web_search", "--json", r#"{"query":}"#])).unwrap_err(),
        BAD_JSON,
        "有双引号的照旧"
    );
    assert_eq!(
        parse(&args(&["web_search", "--json", "niri"])).unwrap_err(),
        BAD_JSON,
        "没花括号的照旧"
    );
}
