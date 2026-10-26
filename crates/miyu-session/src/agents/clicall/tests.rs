//! 给她看的「脚本调工具」（施工 M-4）：只在工具面上有跑命令的工具、有脚本里能调的工具时接上；名单照名字排、MCP 照服务并起来，样子
//! 只列写了的。跑命令照工具自己的声明认，不看名字（施工 BS-1 上）。

use std::sync::Arc;

use miyu_kernel::raw::RawJson;
use miyu_kernel::tool::Access;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Call, Done, Progress, Running, Spec, Tool};

use super::*;

/// 像 `shell` 的假工具：接上的那一段写成 `[名单|样子]`，好断言换进去的是什么。`runs` 是声明不声明跑命令。
struct Shellish(Spec, bool);

/// 出厂的 `shell` 的名字。
const SHELL: &str = miyu_tool::SHELL;

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

    fn runs_commands(&self) -> bool {
        self.1
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
    let shell = Arc::new(Shellish(spec(SHELL, Access::Execute, None), true));
    let bash = Arc::new(Shellish(spec("bash", Access::Execute, None), true));
    let named = Arc::new(Shellish(spec("not_a_shell", Access::Execute, None), false));
    let tool = |name: &str, access, result| {
        Fake::from_spec(spec(name, access, result), Act::Echo) as Arc<dyn Tool>
    };
    Catalog::in_packages([(
        "basesystem",
        vec![
            shell as Arc<dyn Tool>,
            bash as Arc<dyn Tool>,
            named as Arc<dyn Tool>,
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

/// 工具面：`names` 里的照目录的规格，`clicall` 照 `on`，跑命令照目录（[`stamped`]）。
fn face(catalog: &Catalog, names: &[&str], on: bool) -> Vec<ToolEntry> {
    let face = names
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
                runs_commands: false,
                command_only: false,
            }
        })
        .collect();
    let mut face = stamped(face, |name| {
        catalog.get(name).is_some_and(|tool| tool.runs_commands())
    });
    // 跑命令的那件自己不在脚本里调（同出厂的 `shell` 不开 `clicall`）。
    for entry in &mut face {
        entry.clicall = entry.clicall && !entry.runs_commands;
    }
    face
}

fn shell_description(face: &[ToolEntry]) -> &str {
    description(face, SHELL)
}

fn description<'a>(face: &'a [ToolEntry], name: &str) -> &'a str {
    &face
        .iter()
        .find(|entry| entry.name == name)
        .expect("有这一件")
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

/// 不上工具面的（施工 BS-1 中）：`clicall` 写着开也不列，`miyu call` 调不到它。
#[test]
fn tools_off_the_face_are_left_out() {
    let catalog = catalog();
    let mut face = face(&catalog, &[SHELL, "web_fetch", "web_search"], true);
    face.iter_mut()
        .filter(|entry| entry.name == "web_search")
        .for_each(|entry| entry.command_only = true);
    describe(&catalog, &mut face);
    assert_eq!(
        shell_description(&face),
        "shell does things.\n\n[web_fetch|]"
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

/// 跑命令照工具自己的声明认（施工 BS-1 上，设计 44 第三节第 3 条）：别的包带的、名字不叫 `shell` 的声明了照样接上、照样开通道；
/// 没声明的不接，名字是什么都一样。
#[test]
fn whatever_declares_running_commands_gets_the_section_and_the_name_does_not_matter() {
    let catalog = catalog();
    let mut face = face(&catalog, &["bash", "web_fetch"], true);
    describe(&catalog, &mut face);
    assert_eq!(
        description(&face, "bash"),
        "bash does things.\n\n[web_fetch|]"
    );
    assert!(tells(&face));

    // 两件都实现了那一段，只接在声明了跑命令的那件后面。
    let mut both = self::face(&catalog, &["bash", "not_a_shell", "web_fetch"], true);
    describe(&catalog, &mut both);
    // 没声明跑命令的 `not_a_shell` 本身是脚本里能调的一件，照样列在名单里。
    assert_eq!(
        description(&both, "bash"),
        "bash does things.\n\n[not_a_shell, web_fetch|]"
    );
    assert_eq!(
        description(&both, "not_a_shell"),
        "not_a_shell does things."
    );

    let mut face = face_without_runner(&catalog);
    let before = face.clone();
    describe(&catalog, &mut face);
    assert_eq!(face, before, "没声明跑命令的一件都不接");
    assert!(!tells(&face));
}

/// 只有一件没声明跑命令的工具加上能调的工具：没有通道。
fn face_without_runner(catalog: &Catalog) -> Vec<ToolEntry> {
    face(catalog, &["not_a_shell", "web_fetch"], true)
}

/// 以前造的快照里没有「跑命令」这一格：照目录补上以后和新造的一样，`shell` 照旧接上、照旧要 system 那一句。
#[test]
fn an_old_snapshot_face_is_stamped_from_the_catalog() {
    let catalog = catalog();
    let fresh = face(&catalog, &[SHELL, "web_fetch"], true);
    let mut old = fresh.clone();
    for entry in &mut old {
        entry.runs_commands = false;
    }
    assert!(!tells(&old), "没补的时候认不出");
    let stamped = stamped(old, |name| {
        catalog.get(name).is_some_and(|tool| tool.runs_commands())
    });
    assert_eq!(stamped, fresh);
    assert!(tells(&stamped));
}
