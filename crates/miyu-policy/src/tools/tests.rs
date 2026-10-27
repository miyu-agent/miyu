//! 快照里的工具面：照名字排、读回来一样、没有工具的字节不变、造策略时拆成两份、重名的造不出。

use miyu_kernel::history::History;

use super::*;
use crate::test_support::*;

/// 参数格式带着空白：原样进快照、原样进 tools 数组。
const READ_PARAMETERS: &str = r#"{ "type": "object", "properties": {"path": {"type": "string"}} }"#;

fn entry(name: &str, access: Access, parameters: &str) -> ToolEntry {
    ToolEntry {
        name: name.to_string(),
        description: format!("The {name} tool."),
        parameters: serde_json::from_str::<RawJson>(parameters).unwrap(),
        access,
        clicall: false,
        runs_commands: false,
        command_only: false,
    }
}

/// 登记的先后倒过来：read 在前、edit 在后。
fn two() -> Vec<ToolEntry> {
    vec![
        entry("read", Access::Read, READ_PARAMETERS),
        entry("edit", Access::Write, r#"{"type":"object"}"#),
    ]
}

fn names<'a>(names: impl Iterator<Item = &'a String>) -> Vec<&'a str> {
    names.map(String::as_str).collect()
}

#[test]
fn the_tools_are_kept_by_name_and_read_back() {
    let snapshot = engineer().with_tools(two());
    assert_eq!(
        names(snapshot.tools.iter().map(|tool| &tool.name)),
        ["edit", "read"]
    );
    let text = String::from_utf8(snapshot.to_bytes()).unwrap();
    let face = concat!(
        r#""system":"You are a helpful software engineer.","tools":["#,
        r#"{"name":"edit","description":"The edit tool.","parameters":{"type":"object"},"access":"write"},"#,
        r#"{"name":"read","description":"The read tool.","parameters":{ "type": "object", "properties": {"path": {"type": "string"}} },"access":"read"}"#,
        r#"],"core":{"#,
    );
    assert!(text.contains(face), "{text}");
    let back = Snapshot::from_bytes(&snapshot.to_bytes()).unwrap();
    assert_eq!(back, snapshot);
    assert_eq!(back.tools[1].parameters.get(), READ_PARAMETERS);
    // 交进来的先后不影响字节。
    let mut reversed = two();
    reversed.reverse();
    assert_eq!(
        engineer().with_tools(reversed).to_bytes(),
        snapshot.to_bytes()
    );
}

#[test]
fn a_snapshot_without_tools_has_the_bytes_it_had_before() {
    let before = engineer();
    let text = String::from_utf8(before.to_bytes()).unwrap();
    assert!(!text.contains("\"tools\""), "{text}");
    let empty = engineer().with_tools(Vec::new());
    assert_eq!(empty.to_bytes(), before.to_bytes());
    assert_eq!(empty.hash(), before.hash());
    // M3 造的会话，快照里没有这一格：照旧读得回来。
    let back = Snapshot::from_bytes(&before.to_bytes()).unwrap();
    assert!(back.tools.is_empty());
}

#[test]
fn the_policy_gets_the_face_and_the_rules() {
    let policy = engineer().with_tools(two()).policy().unwrap();
    assert_eq!(names(policy.tools.keys()), ["edit", "read"]);
    let read = &policy.tools["read"];
    assert_eq!(read.access, Access::Read);
    assert_eq!(read.parameters.get(), READ_PARAMETERS);
    assert_eq!(policy.tools["edit"].access, Access::Write);
    let request = policy.assembler.assemble(&History::default());
    assert_eq!(
        names(request.tools.iter().map(|tool| &tool.name)),
        ["edit", "read"]
    );
    assert_eq!(request.tools[1].description, "The read tool.");
    assert_eq!(request.tools[1].parameters.get(), READ_PARAMETERS);
}

/// 以前造的快照（施工 L-1 到 FL-1 之间）：工具那一格带 `"need_load":true`、冻着先加载用的字的，照旧读得进；造策略时不认，
/// 照完整格式发，没有 `load_tools`（施工 FL-1，工具一律全量）。
#[test]
fn an_old_snapshot_with_tools_to_load_is_sent_whole() {
    let text = String::from_utf8(engineer().with_tools(two()).to_bytes()).unwrap();
    let read = r#""access":"read""#;
    assert_eq!(text.matches(read).count(), 1, "{text}");
    let text = text.replace(read, r#""access":"read","need_load":true"#);
    let core = r#""core":{"#;
    assert_eq!(text.matches(core).count(), 1, "{text}");
    let text = text.replace(
        core,
        r#""core":{"loading":{"description":"Load tools.","parameters":{"type":"object"},"unknown":"u","bad_args":"b","contract":"c"},"#,
    );
    let old = Snapshot::from_bytes(text.as_bytes()).expect("以前的快照照旧读得进");
    let request = old
        .policy()
        .unwrap()
        .assembler
        .assemble(&History::default());
    assert_eq!(
        names(request.tools.iter().map(|tool| &tool.name)),
        ["edit", "read"],
        "没有 load_tools"
    );
    assert_eq!(
        request.tools[1].parameters.get(),
        READ_PARAMETERS,
        "完整的参数"
    );
}

/// 脚本里能调（施工 M-2）：开的写 `"clicall":true`、交给内核的规则；以前的快照没有这一格，读成不能，写回去一字不差。
#[test]
fn clicall_is_written_only_when_on_and_old_snapshots_keep_their_bytes() {
    let old = engineer().with_tools(two());
    let bytes = old.to_bytes();
    assert!(
        !String::from_utf8(bytes.clone())
            .unwrap()
            .contains("clicall")
    );
    let back = Snapshot::from_bytes(&bytes).unwrap();
    assert!(back.tools.iter().all(|tool| !tool.clicall));
    assert_eq!(back.to_bytes(), bytes);
    assert!(!back.policy().unwrap().tools["read"].clicall);

    let mut tools = two();
    tools[0].clicall = true;
    let on = engineer().with_tools(tools);
    let text = String::from_utf8(on.to_bytes()).unwrap();
    assert!(
        text.contains(r#""access":"read","clicall":true}"#),
        "{text}"
    );
    assert_eq!(Snapshot::from_bytes(&on.to_bytes()).unwrap(), on);
    let policy = on.policy().unwrap();
    assert!(policy.tools["read"].clicall);
    assert!(!policy.tools["edit"].clicall);
}

/// 不上工具面的（施工 BS-1 中）：规则交给内核、带着 `command_only`，tools 数组里没有；写 `"command_only":true`，以前的快照
/// 没有这一格，读成上工具面、写回去一字不差。
#[test]
fn a_tool_off_the_face_is_ruled_but_not_sent() {
    let old = engineer().with_tools(two());
    let bytes = old.to_bytes();
    assert!(
        !String::from_utf8(bytes.clone())
            .unwrap()
            .contains("command_only")
    );
    assert_eq!(Snapshot::from_bytes(&bytes).unwrap().to_bytes(), bytes);

    let mut tools = two();
    tools.push(entry("package", Access::Install, r#"{"type":"object"}"#));
    tools[2].command_only = true;
    let off = engineer().with_tools(tools);
    let text = String::from_utf8(off.to_bytes()).unwrap();
    assert!(text.contains(r#""command_only":true"#), "{text}");
    assert_eq!(Snapshot::from_bytes(&off.to_bytes()).unwrap(), off);
    let policy = off.policy().unwrap();
    assert!(policy.tools["package"].command_only);
    assert!(!policy.tools["read"].command_only);
    let request = policy.assembler.assemble(&History::default());
    assert_eq!(
        names(request.tools.iter().map(|tool| &tool.name)),
        ["edit", "read"]
    );
}

#[test]
fn two_tools_with_one_name_do_not_build() {
    let mut tools = two();
    tools.push(entry("read", Access::Write, r#"{"type":"object"}"#));
    let snapshot = engineer().with_tools(tools);
    let error = snapshot.policy().err().unwrap();
    assert_eq!(error, BuildError::DuplicateTool("read".to_string()));
    assert_eq!(error.to_string(), r#"two tools named "read""#);
    // 从磁盘读回来的也一样：读得回来，造不出策略。
    let back = Snapshot::from_bytes(&snapshot.to_bytes()).unwrap();
    assert_eq!(
        back.policy().err().unwrap(),
        BuildError::DuplicateTool("read".to_string())
    );
}

#[test]
fn the_two_run_texts_name_the_tool() {
    let texts = engineer().run_texts().unwrap();
    assert_eq!(
        texts.unavailable("read").text,
        "The tool \"read\" is not available right now.\n"
    );
    assert_eq!(
        texts.crashed("read").text,
        "The tool \"read\" stopped because of an internal error. It may have been partly done.\n"
    );
    // 名字照样转义：写不出引号和尖括号。
    assert!(!texts.unavailable("a\"<b>").text.contains("<b>"));
    // 给人看的说法，字段原样。
    assert_eq!(
        texts.unavailable("a\"<b>").said,
        Some(Said::new("core/tool-results/unavailable").with("name", "a\"<b>"))
    );
    assert_eq!(
        texts.crashed("read").said,
        Some(Said::new("core/tool-results/crashed").with("name", "read"))
    );
}

#[test]
fn a_broken_run_text_is_named() {
    let mut broken = engineer();
    broken.core.tool_results.crashed = "The tool {nope} broke.".to_string();
    let error = broken.run_texts().unwrap_err();
    assert!(
        matches!(
            error,
            BuildError::Texts {
                which: "executor's tool result texts",
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn a_snapshot_from_before_the_run_texts_reads_back_with_them_empty() {
    let text = String::from_utf8(engineer().to_bytes()).unwrap();
    let old = strip(&strip(&text, "unavailable"), "crashed");
    assert!(!old.contains("\"unavailable\""), "{old}");
    assert!(!old.contains("\"crashed\""), "{old}");
    let back = Snapshot::from_bytes(old.as_bytes()).unwrap();
    assert_eq!(back.core.tool_results.unavailable, "");
    assert_eq!(back.core.tool_results.crashed, "");
    assert_eq!(back.run_texts().unwrap().unavailable("read").text, "");
}

/// 从快照的 JSON 里去掉 `field` 那一格（值是一个字符串）：造出这一格以前的样子。
fn strip(text: &str, field: &str) -> String {
    let key = format!(",\"{field}\":\"");
    let start = text.find(&key).unwrap();
    let value_start = start + key.len();
    let mut end = value_start;
    let bytes = text.as_bytes();
    while bytes[end] != b'"' || bytes[end - 1] == b'\\' {
        end += 1;
    }
    format!("{}{}", &text[..start], &text[end + 1..])
}

/// 随包卸掉了的说「已卸载」（施工 F-5 中）；以前造的快照没有这一句，照「现在用不了」说。
#[test]
fn an_uninstalled_tool_is_named_and_old_snapshots_fall_back() {
    let texts = engineer().run_texts().unwrap();
    assert_eq!(
        texts.uninstalled("read").text,
        "The tool \"read\" was uninstalled.\n"
    );
    assert_eq!(
        texts.uninstalled("read").said,
        Some(Said::new("core/tool-results/uninstalled").with("name", "read"))
    );
    let text = String::from_utf8(engineer().to_bytes()).unwrap();
    let old = strip(&text, "uninstalled");
    let back = Snapshot::from_bytes(old.as_bytes()).unwrap();
    assert_eq!(back.core.tool_results.uninstalled, "");
    assert_eq!(
        back.run_texts().unwrap().uninstalled("read").text,
        "The tool \"read\" is not available right now.\n"
    );
}
