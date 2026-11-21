//! 连上核心以前出的错照界面语言说（施工 EN-1）：真的 `miyu` 程序在英文、中文界面下印的那一句；核心起不来的照它带的代码说
//! （施工 EN-2）。

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
            "miyu: MIYU_HOME 必须是绝对路径，当前为 rel/path\n",
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

/// 施工 EN-2：核心起不来的（运行日志的目录是个文件、数据根里的工作区是个文件），照那一行的代码用界面语言说整句，原因是
/// 系统的原话。
#[test]
fn a_core_that_cannot_start_is_said_in_the_interface_language() {
    let cwd = std::env::temp_dir();
    for (blocked, said) in [
        (
            "var/log",
            "miyu: the core couldn't start: cannot write its log: ",
        ),
        (
            "root/workspace",
            "miyu: the core couldn't start: cannot create folders in the data root: ",
        ),
    ] {
        let home = Home::new();
        let path = home.root.path().join(blocked);
        std::fs::create_dir_all(path.parent().expect("有上一级")).expect("建得了");
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("删得了");
        }
        std::fs::write(&path, "").expect("写得进");
        let output = command(home.root.path(), &cwd, "en_US.UTF-8", &["ask", "hi"])
            .stdin(std::process::Stdio::null())
            .output()
            .expect("跑得起来");
        let shown = stderr(&output);
        assert_eq!(output.status.code(), Some(1), "{shown}");
        assert!(shown.starts_with(said), "{blocked}：{shown}");
        assert!(
            !shown
                .chars()
                .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)),
            "{shown}"
        );
    }
}
