//! 主程序的测试（施工 ACP-6 从 `main.rs` 挪出来：主程序到了 500 行的上限）：快路和 clap 解析得一样，每个子命令印自己的帮助页。

use clap::CommandFactory;

use super::*;

/// 敲 `miyu <path…> -h` 印出来的字。
fn printed(command: &clap::Command, path: &[String]) -> String {
    let words = std::iter::once("miyu")
        .chain(path.iter().map(String::as_str))
        .chain(std::iter::once("-h"));
    let error = command
        .clone()
        .try_get_matches_from(words)
        .expect_err("印帮助");
    assert_eq!(error.kind(), ErrorKind::DisplayHelp, "{path:?}");
    error.to_string()
}

/// `at` 和它下面每一层的子命令里，`-h` 印的不是 `help` 这一页的，照 `miyu pkg pack` 这样记进 `wrong`。
fn differing(
    command: &clap::Command,
    at: &clap::Command,
    path: &[String],
    help: &str,
    wrong: &mut Vec<String>,
) {
    if printed(command, path) != help {
        wrong.push(format!("miyu {}", path.join(" ")));
    }
    for sub in at.get_subcommands() {
        let mut path = path.to_vec();
        path.push(sub.get_name().to_owned());
        differing(command, sub, &path, help, wrong);
    }
}

/// `miyu <words…>` 照 [`dispatch`] 用的那棵命令行树（换好帮助页的）解析出来的样子：跑核心的写成 `core idle=<秒>`，别的
/// 子命令写成 `other`，出错的写成错误的种类和 clap 印的那几行。
fn parsed(words: &[&str]) -> String {
    let command = dressed(Cli::command(), Language::English, String::from("miyu\n"));
    match command
        .try_get_matches_from(std::iter::once("miyu").chain(words.iter().copied()))
        .and_then(|matches| Cli::from_arg_matches(&matches))
    {
        Ok(Cli {
            command:
                Some(Command::Core {
                    idle_seconds,
                    action: None,
                }),
        }) => format!("core idle={idle_seconds:?}"),
        Ok(_) => String::from("other"),
        Err(error) => format!("{:?}: {error}", error.kind()),
    }
}

/// 同 [`parsed`]，只是先走快路：快路认得出的照它，认不出的照 clap。
fn fast(words: &[&str]) -> String {
    let os: Vec<OsString> = words.iter().map(OsString::from).collect();
    match running_core(&os) {
        Some(idle_seconds) => format!("core idle={idle_seconds:?}"),
        None => parsed(words),
    }
}

#[test]
fn the_fast_way_into_the_core_parses_as_clap_does() {
    // 施工 LW-8：快路认的和整棵命令行树认的一样；认不出的交给 clap，报错的字一字不差。
    let cases: &[&[&str]] = &[
        &["core"],
        &["core", "--idle-seconds", "5"],
        &["core", "--idle-seconds", "0"],
        &["core", "--idle-seconds", "+7"],
        &["core", "--idle-seconds", "18446744073709551615"],
        &["core", "--idle-seconds=3600"],
        &["core", "--idle-seconds=+1"],
        // 写错的、要帮助的、管理用的：都该落到 clap。
        &["core", "--idle-seconds", "18446744073709551616"],
        &["core", "--idle-seconds", "-5"],
        &["core", "--idle-seconds", ""],
        &["core", "--idle-seconds", "five"],
        &["core", "--idle-seconds", " 5"],
        &["core", "--idle-seconds="],
        &["core", "--idle-seconds=x"],
        &["core", "--idle-seconds"],
        &["core", "--idle-seconds", "5", "--idle-seconds", "6"],
        &["core", "--idle-second", "5"],
        &["core", "--idle", "5"],
        &["core", "--help"],
        &["core", "-h"],
        &["core", "--version"],
        &["core", "-V"],
        &["core", "--idle-seconds", "5", "--help"],
        &["core", "start"],
        &["core", "status", "--format", "json"],
        &["core", "stop"],
        &["core", "--idle-seconds", "5", "stop"],
        &["core", "restart"],
        &["core", "--"],
        &["core", "-"],
        &["Core"],
        &["cor"],
        &["core", "core"],
    ];
    for words in cases {
        assert_eq!(fast(words), parsed(words), "{words:?}");
    }
    // 快路真的认得出跑核心的几种写法，别的一个都不认。
    let words = |list: &[&str]| list.iter().map(OsString::from).collect::<Vec<_>>();
    assert_eq!(running_core(&words(&["core"])), Some(None));
    assert_eq!(
        running_core(&words(&["core", "--idle-seconds", "5"])),
        Some(Some(5))
    );
    assert_eq!(
        running_core(&words(&["core", "--idle-seconds=9"])),
        Some(Some(9))
    );
    assert_eq!(running_core(&words(&["core", "--help"])), None);
    assert_eq!(running_core(&words(&["core", "start"])), None);
    assert_eq!(running_core(&[]), None);
}

#[cfg(unix)]
#[test]
fn the_fast_way_leaves_words_that_are_not_utf8_to_clap() {
    use std::os::unix::ffi::OsStringExt;
    let words = [OsString::from("core"), OsString::from_vec(vec![0xff])];
    assert_eq!(running_core(&words), None);
}

#[test]
fn every_subcommand_prints_the_page_of_its_command() {
    // 漏挂的印 clap 照源码注释生成的那一页：英文界面里是中文、带施工号（审计 #037，施工 HLP-1）。
    for language in [Language::Chinese, Language::English] {
        let command = dressed(Cli::command(), language, String::from("miyu\n"));
        assert_eq!(printed(&command, &[]), "miyu\n");
        let mut wrong = Vec::new();
        for top in command.get_subcommands() {
            let path = [top.get_name().to_owned()];
            match PAGES.iter().find(|(name, _)| *name == top.get_name()) {
                Some((_, which)) => {
                    differing(&command, top, &path, page(language, *which), &mut wrong);
                }
                None => wrong.push(format!("miyu {}（没有帮助页）", path[0])),
            }
        }
        assert_eq!(wrong, Vec::<String>::new(), "{language:?}");
    }
}
