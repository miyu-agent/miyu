//! 真跑 `miyu kb`（施工 K-7 下，`docs/blueprint/cli/kb.md`）：`kb -h`、各个子命令的 `-h`、`help kb` 印的都是那一页，跟着界面
//! 语言；参数写错的退出码 2：子命令写错、少了库、少了文件、少了目录、少了词。

use std::path::Path;
use std::process::{Command, Output};

use crate::support::{Home, MIYU};
use miyu_cli::help::{Page, page};
use miyu_cli::language::Language;

/// 在数据根 `root` 上跑 `miyu <args>`：界面语言是 `lang`。
fn miyu(root: &Path, lang: &str, args: &[&str]) -> Output {
    Command::new(MIYU)
        .args(args)
        .env("MIYU_HOME", root)
        .envs(crate::support::offline(root))
        .env("MIYU_RESOURCES", crate::support::resources())
        .env("LANG", lang)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("XDG_RUNTIME_DIR")
        .output()
        .expect("跑得起来")
}

#[test]
fn every_help_is_the_page_in_the_language() {
    let home = Home::new();
    for (lang, language) in [("zh_CN.UTF-8", Language::Chinese), ("C", Language::English)] {
        for args in [
            &["kb", "-h"][..],
            &["kb", "--help"],
            &["help", "kb"],
            &["kb", "list", "-h"],
            &["kb", "add", "--help"],
            &["kb", "mount", "-h"],
            &["kb", "delete", "-h"],
        ] {
            let output = miyu(home.root.path(), lang, args);
            assert!(output.status.success(), "{args:?}：{output:?}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                page(language, Page::Kb),
                "{lang} {args:?}"
            );
        }
    }
}

#[test]
fn wrong_arguments_are_exit_code_2() {
    let home = Home::new();
    for args in [
        &["kb", "drop"][..],
        &["kb", "search"],
        &["kb", "read", "notes"],
        &["kb", "new"],
        &["kb", "add", "notes"],
        &["kb", "mount", "notes"],
        &["kb", "delete"],
        &["kb", "list", "--page", "x"],
        &["kb", "list", "--format", "xml"],
    ] {
        let output = miyu(home.root.path(), "C", args);
        assert_eq!(output.status.code(), Some(2), "{args:?}：{output:?}");
    }
}
