//! 扩展放在 system 尾部的那几段（施工 X-20，`docs/designs/34-表情包.md` 第五节第 5 条）：照这一刻的工具面挑出有工具在面上的
//! 包，它说过的照它说的，没说过的照快照里钉的；工具不在面上的包不带。

use std::collections::BTreeMap;

use miyu_policy::ToolEntry;
use miyu_tool::Catalog;

/// 这一轮要放的那几段：`pinned` 是快照里钉的，`asked` 是扩展这一刻说的（经端口问到的，`None` 是去掉了）。
pub(crate) fn resolve(
    catalog: &Catalog,
    face: &[ToolEntry],
    pinned: &BTreeMap<String, String>,
    asked: &BTreeMap<String, Option<String>>,
) -> BTreeMap<String, String> {
    face.iter()
        .filter_map(|tool| catalog.package_of(&tool.name))
        .filter_map(|package| {
            let text = match asked.get(package) {
                Some(said) => said.clone(),
                None => pinned.get(package).cloned(),
            };
            Some((package.to_string(), text?))
        })
        .chain(proto())
        .collect()
}

/// 原型 M-0（设计 39 第十节）：照核心的环境变量 `MIYU_PROTO_SYS` 在 system 尾部放一段引导，`full` 写全用法，`short` 只一句。
fn proto() -> Option<(String, String)> {
    const FULL: &str = r#"Scripting tools: some tools can also be called from shell commands with miyu call <tool> --<param> <value>... (or --json '<object>', --json - for stdin). The result goes to stdout: JSON when the tool has structured output, otherwise text. Only what the script prints comes back to you. When a task needs the same tool many times, or a loop over pages or items, and you only need a summary (counts, filters, comparisons), write one shell script with miyu call instead of making many direct calls. Exit codes: 1 failed, 2 bad usage, 3 interrupted, 4 needs approval (call the tool directly). miyu call --schema <tool> prints its parameters. miyu call works only for: web_search, web_fetch, memory_search, kb_search, kb_read, mcp__*. Call every tool directly as usual.
JSON shapes: web_search {"kind":"links","links":[{"title","url","snippet","site","date"}]}; memory_search {"memories":[{"id","class","date","text"}],"turns":[{"date","session","text"}]}; kb_search {"hits":[{"kb","path","heading","snippet","page"}]}; kb_read {"kb","path","page","pages","text"}. web_fetch and MCP tools without structured output print text."#;
    const SHORT: &str = "When a task needs the same tool many times, or a loop over pages or items, and you only need a summary (counts, filters, comparisons), write one shell script with miyu call (see the shell tool) instead of making many direct calls.";
    let text = match std::env::var("MIYU_PROTO_SYS").ok()?.as_str() {
        "full" => FULL,
        "short" => SHORT,
        _ => return None,
    };
    Some(("zz-proto-miyu-call".to_string(), text.to_string()))
}
