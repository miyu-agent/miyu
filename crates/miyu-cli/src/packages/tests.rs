//! 软件包加的子命令（施工 9-2）：什么时候转交、帮助页多的那一节、没装程序时说什么。真的换成程序、退出码照传，在
//! `crates/miyu/tests/packages.rs` 照真二进制走一遍（换成别的程序会把测试进程换掉）。

use super::*;

fn added(name: &str, program: &str) -> Added {
    Added {
        name: name.to_string(),
        program: program.to_string(),
        args: Vec::new(),
        about: Phrases::from([
            ("en".to_string(), "Open the terminal interface".to_string()),
            ("zh".to_string(), "打开终端界面".to_string()),
        ]),
        manifest: PathBuf::from("/data/packages/tui/package.toml"),
        resources: PathBuf::from("/usr/share/miyu"),
        state: PathBuf::from("/data/state/packages/tui"),
    }
}

fn args(words: &[&str]) -> Vec<OsString> {
    std::iter::once("miyu")
        .chain(words.iter().copied())
        .map(OsString::from)
        .collect()
}

#[test]
fn only_a_package_command_is_forwarded() {
    let list = [added("tui", "miyu-no-such-program-anywhere")];
    let main = Path::new("/nowhere/miyu");
    let mut err = Vec::new();
    for words in [
        &[][..],
        &["ask", "hi"],
        &["-h"],
        &["web"],
        &["help"],
        &["help", "ask"],
        &["nope"],
    ] {
        assert_eq!(
            forward(&args(words), &list, main, Language::English, &mut err),
            None,
            "{words:?}"
        );
    }
    assert!(err.is_empty());
}

#[test]
fn a_missing_program_is_named_with_its_manifest() {
    let list = [added("tui", "miyu-no-such-program-anywhere")];
    let main = Path::new("/nowhere/miyu");
    for (words, language, said) in [
        (
            &["tui", "--x"][..],
            Language::English,
            "miyu-no-such-program-anywhere not found: /data/packages/tui/package.toml says miyu tui runs it",
        ),
        (
            &["help", "tui"][..],
            Language::Chinese,
            "没找到 miyu-no-such-program-anywhere：清单 /data/packages/tui/package.toml 说 miyu tui 由它跑",
        ),
    ] {
        let mut err = Vec::new();
        assert_eq!(
            forward(&args(words), &list, main, language, &mut err),
            Some(1)
        );
        let text = String::from_utf8(err).unwrap();
        assert!(text.starts_with(said), "{text}");
    }
}

#[test]
fn the_help_page_gets_a_section_after_the_commands() {
    use crate::help::{Page, page};
    let list = [added("tui", "miyu-tui"), added("onebot", "miyu-onebot")];
    let section = help_section(Language::Chinese, &list);
    assert_eq!(
        section,
        "软件包加的命令：\n  tui                 打开终端界面\n  onebot              打开终端界面\n"
    );
    assert_eq!(
        help_section(Language::English, &list[..1]),
        "Commands from packages:\n  tui                 Open the terminal interface\n"
    );
    assert_eq!(help_section(Language::English, &[]), "");
    // 说明和总表里的命令从同一列开始（第 22 列）。
    let at = |text: &str, word: &str| {
        let line = text
            .lines()
            .find(|line| line.contains(word))
            .expect("有这行");
        // 中文字占两列，别的一列。
        line[..line.find(word).expect("有这个词")]
            .chars()
            .map(|c| if c.is_ascii() { 1 } else { 2 })
            .sum::<usize>()
    };
    let table = page(Language::Chinese, Page::Miyu);
    assert_eq!(at(&section, "打开终端界面"), 22);
    assert_eq!(at(table, "发送消息并打印回复"), 22);
    let sample = "用法：miyu <命令>\n\n对话：\n  ask   说\n\n系统：\n  web   开\n\n选项：\n  -c\n";
    assert_eq!(with_section(sample, ""), sample);
    let with = with_section(sample, &section);
    assert!(
        with.contains("  web   开\n\n软件包加的命令：\n  tui"),
        "接在最后一组命令后面、「选项」前面：{with}"
    );
    assert!(with.contains("打开终端界面\n\n选项：\n  -c"), "{with}");
    // 总表接上以后：中英文都在最后一组命令后面、「选项」前面。
    let english = help_section(Language::English, &list[..1]);
    let shown = with_section(page(Language::English, Page::Miyu), &english);
    assert!(
        shown.contains("(Windows)\n\nCommands from packages:\n  tui")
            && shown.contains("interface\n\nOptions:\n  -h, --help"),
        "{shown}"
    );
    let shown = with_section(table, &section);
    assert!(
        shown.contains("（Windows）\n\n软件包加的命令：\n  tui")
            && shown.contains("打开终端界面\n  onebot")
            && shown.contains("打开终端界面\n\n选项：\n  -h, --help"),
        "{shown}"
    );
    // 找不到「选项」那一节的页：照原来的办法，接在第二个空行那里。
    let bare = "用法：miyu <命令>\n\n命令：\n  ask   说\n\nask 的选项：\n  -c\n";
    let with = with_section(bare, &section);
    assert!(
        with.contains("  ask   说\n\n软件包加的命令：\n  tui"),
        "{with}"
    );
    assert!(with.contains("打开终端界面\n\nask 的选项："), "{with}");
}

/// 跑包的程序时环境多三个：`MIYU_PACKAGE_DIR` 是这个包的文件夹（施工 S-6 上），`MIYU_PACKAGE_STATE` 是它的状态目录（施工
/// F-15），`MIYU_RESOURCES` 是资源目录（施工 S-6 中）；参数照原样。真的换成程序在
/// `crates/miyu/tests/packages.rs` 照真二进制走一遍（Unix）。
#[test]
fn the_program_gets_its_package_folder() {
    let dir = Path::new("/data/packages/tui");
    let state = Path::new("/data/state/packages/tui");
    let resources = Path::new("/usr/share/miyu");
    let command = command(
        &Launch::direct(std::path::PathBuf::from("/bin/miyu-tui")),
        &[OsString::from("--x")],
        (dir, state),
        resources,
    );
    assert_eq!(command.get_program(), "/bin/miyu-tui");
    assert_eq!(command.get_args().collect::<Vec<_>>(), ["--x"]);
    // Windows 上照 `#!` 起的（施工 X-34）：解释器是程序，脚本排在人敲的参数前面。
    let script = Launch {
        program: std::path::PathBuf::from("python"),
        args: vec![OsString::from("/pkg/bin/miyu-tui")],
    };
    let shebang = super::command(&script, &[OsString::from("--x")], (dir, state), resources);
    assert_eq!(shebang.get_program(), "python");
    assert_eq!(
        shebang.get_args().collect::<Vec<_>>(),
        ["/pkg/bin/miyu-tui", "--x"]
    );
    let envs: Vec<_> = command.get_envs().collect();
    // 主程序的真实位置（施工 G-15）：这里跑的是测试程序。
    let program = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .expect("测试程序在");
    assert_eq!(
        envs,
        [
            (
                std::ffi::OsStr::new("MIYU_PACKAGE_DIR"),
                Some(dir.as_os_str())
            ),
            (
                std::ffi::OsStr::new("MIYU_PACKAGE_STATE"),
                Some(state.as_os_str())
            ),
            (
                std::ffi::OsStr::new("MIYU_PROGRAM"),
                Some(program.as_os_str())
            ),
            (
                std::ffi::OsStr::new("MIYU_RESOURCES"),
                Some(resources.as_os_str())
            ),
        ],
        "自己装的包程序在包的 bin/ 里，资源目录也得交（施工 S-6 中）；主程序在哪也交（施工 G-15）"
    );
}

#[test]
fn the_manifest_args_go_before_what_was_typed() {
    let mut shellhook = added("shellhook", "miyu-tui");
    shellhook.args = vec!["shellhook".to_string()];
    let list = [added("tui", "miyu-tui"), shellhook];
    let passed =
        |words: &[&str]| target(&args(words), &list).map(|(one, rest)| (one.name.clone(), rest));
    let os = |words: &[&str]| words.iter().map(OsString::from).collect::<Vec<_>>();
    assert_eq!(
        passed(&["shellhook", "install", "fish"]),
        Some((
            "shellhook".to_string(),
            os(&["shellhook", "install", "fish"])
        ))
    );
    assert_eq!(
        passed(&["help", "shellhook"]),
        Some(("shellhook".to_string(), os(&["shellhook", "--help"])))
    );
    assert_eq!(
        passed(&["tui", "--page", "config"]),
        Some(("tui".to_string(), os(&["--page", "config"]))),
        "没写 args 的照旧"
    );
}
