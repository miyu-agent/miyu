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
        manifest: PathBuf::from("/data/home/admin/packages/tui/package.toml"),
        resources: PathBuf::from("/usr/share/miyu"),
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
            "miyu-no-such-program-anywhere not found: /data/home/admin/packages/tui/package.toml says miyu tui runs it",
        ),
        (
            &["help", "tui"][..],
            Language::Chinese,
            "没找到 miyu-no-such-program-anywhere：清单 /data/home/admin/packages/tui/package.toml 说 miyu tui 由它跑",
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
    let list = [added("tui", "miyu-tui"), added("onebot", "miyu-onebot")];
    let section = help_section(Language::Chinese, &list);
    assert_eq!(
        section,
        "软件包加的命令：\n  tui                   打开终端界面\n  onebot                打开终端界面\n"
    );
    assert_eq!(
        help_section(Language::English, &list[..1]),
        "Commands from packages:\n  tui                   Open the terminal interface\n"
    );
    assert_eq!(help_section(Language::English, &[]), "");
    let page = "用法：miyu <命令>\n\n命令：\n  ask   说\n\nask 的选项：\n  -c\n";
    assert_eq!(with_section(page, ""), page);
    let with = with_section(page, &section);
    assert!(
        with.contains("  ask   说\n\n软件包加的命令：\n  tui"),
        "接在「命令」那一节后面：{with}"
    );
    assert!(with.contains("打开终端界面\n\nask 的选项："), "{with}");
}

/// 跑包的程序时环境多两个：`MIYU_PACKAGE_DIR` 是这个包的文件夹（施工 S-6 上），`MIYU_RESOURCES` 是资源目录（施工 S-6 中）；参数照原样。真的换成程序在
/// `crates/miyu/tests/packages.rs` 照真二进制走一遍（Unix）。
#[test]
fn the_program_gets_its_package_folder() {
    let dir = Path::new("/data/home/admin/packages/tui");
    let resources = Path::new("/usr/share/miyu");
    let command = command(
        Path::new("/bin/miyu-tui"),
        &[OsString::from("--x")],
        dir,
        resources,
    );
    assert_eq!(command.get_program(), "/bin/miyu-tui");
    assert_eq!(command.get_args().collect::<Vec<_>>(), ["--x"]);
    let envs: Vec<_> = command.get_envs().collect();
    assert_eq!(
        envs,
        [
            (
                std::ffi::OsStr::new("MIYU_PACKAGE_DIR"),
                Some(dir.as_os_str())
            ),
            (
                std::ffi::OsStr::new("MIYU_RESOURCES"),
                Some(resources.as_os_str())
            ),
        ],
        "自己装的包程序在包的 bin/ 里，资源目录也得交（施工 S-6 中）"
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
