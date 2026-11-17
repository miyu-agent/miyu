//! 连上核心以前出的错照界面语言说（施工 EN-1）：真的 `miyu` 程序在英文、中文界面下印的那一句。

use crate::support::Home;
use crate::support::cli::{command, stderr};

/// 施工 EN-1：`MIYU_HOME` 写了相对路径的，命令行照界面语言说（原来一律中文原话）。
#[test]
fn a_relative_miyu_home_is_said_in_the_interface_language() {
    let home = Home::new();
    let cwd = std::env::temp_dir();
    for (lang, said) in [
        (
            "en_US.UTF-8",
            "miyu: MIYU_HOME must be an absolute path, got rel/path\n",
        ),
        (
            "zh_CN.UTF-8",
            "miyu: MIYU_HOME 要写绝对路径，写的是 rel/path\n",
        ),
    ] {
        let output = command(home.root.path(), &cwd, lang, &["check"])
            .env("MIYU_HOME", "rel/path")
            .output()
            .expect("跑得起来");
        assert_eq!(
            (output.status.code(), stderr(&output).as_str()),
            (Some(1), said)
        );
    }
}
