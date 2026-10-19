//! 一个包加几个子命令（施工 F-13）：`[[command]]` 照写的先后，`args` 可以不写，同一个包里不能重名。

use crate::package::{Code, read};

/// 终端界面的包：自己一个，再加 `shellhook`（同一个程序，转交时前面加 `shellhook`）。
const TWO: &str = r#"[package]
protocol = [1, 1]
name = { en = "Terminal" }

[[command]]
name = "tui"
program = "miyu-tui"
about = { en = "Open the terminal interface" }

[[command]]
name = "shellhook"
program = "miyu-tui"
args = ["shellhook"]
about = { en = "Talk to Miyu from your shell" }

[ui]
"#;

#[test]
fn a_package_can_add_several_commands() {
    let manifest = read(TWO).expect("读得成");
    let names: Vec<&str> = manifest
        .commands
        .iter()
        .map(|command| command.name.as_str())
        .collect();
    assert_eq!(names, ["tui", "shellhook"]);
    assert_eq!(
        manifest.command().map(|command| command.name.as_str()),
        Some("tui"),
        "第一个是这个包的程序那一个"
    );
    let shellhook = manifest.command_named("shellhook").expect("有");
    assert_eq!(shellhook.args, ["shellhook"]);
    assert_eq!(shellhook.program, "miyu-tui");
    assert_eq!(shellhook.line, Some(11));
    assert!(manifest.command_named("tui").expect("有").args.is_empty());
}

#[test]
fn a_name_written_twice_in_one_package_is_refused() {
    let twice = TWO.replace("name = \"shellhook\"", "name = \"tui\"");
    let problem = read(&twice).expect_err("同一个包里重名");
    assert_eq!(problem.code, Code::CommandTaken);
    assert_eq!(problem.line, Some(11), "报在后一个");
    assert_eq!(problem.detail, "command.tui");
}

#[test]
fn args_must_be_a_list_of_text() {
    let one = TWO.replace("args = [\"shellhook\"]", "args = \"shellhook\"");
    let problem = read(&one).expect_err("不是数组");
    assert_eq!(problem.code, Code::NotTexts);
    let unknown = TWO.replace("args = [\"shellhook\"]", "argv = [\"shellhook\"]");
    assert_eq!(read(&unknown).expect_err("不认识").code, Code::UnknownKey);
}
