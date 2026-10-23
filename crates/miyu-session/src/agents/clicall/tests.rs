//! 给她看的「脚本调工具」（施工 M-4）：只在工具面上有 `shell`、有脚本里能调的工具时接上；名单照名字排、MCP 照服务并起来，样子只列
//! 写了的。

use std::sync::Arc;

use miyu_kernel::raw::RawJson;
use miyu_kernel::tool::Access;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Call, Done, Progress, Running, Spec, Tool};

use super::*;

/// 像 `shell` 的假工具：接上的那一段写成 `[名单|样子]`，好断言换进去的是什么。
struct Shellish(Spec);

impl Tool for Shellish {
    fn spec(&self) -> &Spec {
        &self.0
    }

    fn run(&self, _call: Call, _progress: Progress) -> Running<'_> {
        Box::pin(async { Done::ok("") })
    }

    fn clicall_section(&self, tools: &str, shapes: &str) -> Option<String> {
        Some(format!("[{tools}|{shapes}]"))
    }
}

fn spec(name: &str, access: Access, result: Option<&str>) -> Spec {
    Spec {
        name: name.to_string(),
        description: format!("{name} does things."),
        parameters: RawJson::of(&serde_json::json!({"type": "object"})).expect("是 JSON"),
        access,
        need_load: false,
        clicall: true,
        result: result.map(str::to_string),
        plugin: None,
    }
}

fn catalog() -> Catalog {
    let shell = Arc::new(Shellish(spec(SHELL, Access::Execute, None)));
    let tool = |name: &str, access, result| {
        Fake::from_spec(spec(name, access, result), Act::Echo) as Arc<dyn Tool>
    };
    Catalog::in_packages([(
        "basesystem",
        vec![
            shell as Arc<dyn Tool>,
            tool(
                "web_search",
                Access::Network,
                Some(r#"{"links":[{"url"}]}"#),
            ),
            tool("web_fetch", Access::Network, None),
            tool("write", Access::Write, None),
            tool("mcp__github__get_issue", Access::Network, None),
            tool("mcp__github__list_issues", Access::Network, None),
            tool("mcp__notes__find", Access::Read, None),
        ],
    )])
    .expect("合写法")
}

/// 工具面：`names` 里的照目录的规格，`clicall` 照 `on`。
fn face(catalog: &Catalog, names: &[&str], on: bool) -> Vec<ToolEntry> {
    names
        .iter()
        .map(|name| {
            let spec = catalog.get(name).expect("目录里有").spec().clone();
            ToolEntry {
                name: spec.name,
                description: spec.description,
                parameters: spec.parameters,
                access: spec.access,
                need_load: false,
                clicall: on && *name != SHELL,
            }
        })
        .collect()
}

fn shell_description(face: &[ToolEntry]) -> &str {
    &face
        .iter()
        .find(|entry| entry.name == SHELL)
        .expect("有 shell")
        .description
}

#[test]
fn the_shell_gets_the_section_with_sorted_names_merged_mcp_and_declared_shapes() {
    let catalog = catalog();
    let mut face = face(
        &catalog,
        &[
            SHELL,
            "web_search",
            "web_fetch",
            "mcp__github__get_issue",
            "mcp__github__list_issues",
            "mcp__notes__find",
        ],
        true,
    );
    describe(&catalog, &mut face);
    assert_eq!(
        shell_description(&face),
        "shell does things.\n\n[mcp__github__*, mcp__notes__*, web_fetch, web_search|web_search {\"links\":[{\"url\"}]}]"
    );
    assert!(tells(&face));
    let web = face
        .iter()
        .find(|entry| entry.name == "web_search")
        .unwrap();
    assert_eq!(
        web.description, "web_search does things.",
        "别的工具的说明不动"
    );
}

#[test]
fn tools_scripts_cannot_call_are_left_out() {
    let catalog = catalog();
    let mut face = face(&catalog, &[SHELL, "write", "web_fetch"], true);
    describe(&catalog, &mut face);
    assert_eq!(
        shell_description(&face),
        "shell does things.\n\n[web_fetch|]",
        "写入的开了 clicall 也调不成，不列"
    );
}

#[test]
fn nothing_callable_leaves_the_face_and_system_alone() {
    let catalog = catalog();
    let mut off = face(&catalog, &[SHELL, "web_search"], false);
    describe(&catalog, &mut off);
    assert_eq!(shell_description(&off), "shell does things.");
    assert!(!tells(&off));

    let mut written = face(&catalog, &[SHELL, "write"], true);
    describe(&catalog, &mut written);
    assert_eq!(shell_description(&written), "shell does things.");
    assert!(!tells(&written));
}

#[test]
fn without_the_shell_there_is_no_channel_to_tell_about() {
    let catalog = catalog();
    let mut face = face(&catalog, &["web_search", "web_fetch"], true);
    let before = face.clone();
    describe(&catalog, &mut face);
    assert_eq!(face, before);
    assert!(!tells(&face));
}

#[test]
fn mcp_names_are_listed_by_server() {
    assert_eq!(listed("mcp__github__get_issue"), "mcp__github__*");
    assert_eq!(listed("mcp__a__b__c"), "mcp__a__*");
    assert_eq!(listed("web_search"), "web_search");
    assert_eq!(listed("mcp__odd"), "mcp__odd", "写法不对的照原样");
}
