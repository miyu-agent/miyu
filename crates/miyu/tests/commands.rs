//! 命令行的规矩（`docs/designs/22-命令行.md` 第二节，施工 3-9 上）：不认识的子命令就报错、退出码 2，绝不当成
//! 对话发给核心；给人看的话跟着界面语言。

use std::process::{Command, Output};

use crate::support::{Home, MIYU};
use miyu_cli::help::{Page, page};
use miyu_cli::language::Language;

/// 在临时的数据根上跑 `miyu <args>`，界面语言是 `lang`。
fn miyu(home: &Home, lang: &str, args: &[&str]) -> Output {
    Command::new(MIYU)
        .args(args)
        .env("MIYU_HOME", home.root.path())
        .envs(crate::support::offline(home.root.path()))
        .env("LANG", lang)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .output()
        .expect("跑得起来")
}

#[test]
fn an_unknown_command_is_refused_and_never_sent() {
    let home = Home::new();
    let output = miyu(&home, "zh_CN.UTF-8", &["hello"]);
    assert_eq!(output.status.code(), Some(2));
    let said = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        said.trim_end(),
        "miyu: 未知命令 'hello'\n提示：运行 'miyu --help' 查看可用命令"
    );
    assert!(output.stdout.is_empty());
    assert!(!home.root.run().join("socket").exists(), "没拉起核心");
    assert!(home.core_log().is_empty(), "核心没起来过");

    let output = miyu(&home, "C", &["hello"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim_end(),
        "miyu: unknown command 'hello'\nhint: see 'miyu --help'"
    );
}

/// `miyu sandbox` 去掉了（施工 LV-2，Windows 的沙盒用户撤了）：照不认识的命令说，不拉起核心。
#[test]
fn the_sandbox_command_is_gone() {
    let home = Home::new();
    for args in [&["sandbox", "setup"][..], &["sandbox", "remove"]] {
        let output = miyu(&home, "C", args);
        assert_eq!(output.status.code(), Some(2), "{args:?}：{output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).trim_end(),
            "miyu: unknown command 'sandbox'\nhint: see 'miyu --help'"
        );
        assert!(output.stdout.is_empty(), "{output:?}");
    }
    assert!(home.core_log().is_empty(), "核心没起来过");
}

#[test]
fn plain_miyu_off_a_terminal_prints_the_help() {
    // 不在终端里（这里标准输入、输出都接着管道）：不拉起界面，印帮助、退出码 2（施工 9-3）。
    let home = Home::new();
    let output = miyu(&home, "zh_CN.UTF-8", &[]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        page(Language::Chinese, Page::Miyu),
        "{output:?}"
    );
    assert!(output.stderr.is_empty(), "{output:?}");
    assert!(!home.root.run().join("socket").exists(), "没拉起核心");
}

#[test]
fn the_help_is_the_page_and_the_version_says_miyu() {
    let home = Home::new();
    // `-h`、`--help`、`help` 印的都是自己写的那一页，一个字节不差（施工 4-11）。
    for (lang, language) in [("zh_CN.UTF-8", Language::Chinese), ("C", Language::English)] {
        for args in [&["-h"][..], &["--help"], &["help"]] {
            let output = miyu(&home, lang, args);
            assert!(output.status.success(), "{args:?}：{output:?}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                page(language, Page::Miyu),
                "{lang} {args:?}"
            );
            assert!(output.stderr.is_empty(), "{output:?}");
        }
    }
    let output = miyu(&home, "C", &["--version"]);
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("miyu "));
}

#[test]
fn a_mistake_is_one_error_with_hints_and_exit_code_2() {
    let home = Home::new();
    let cases: [(&[&str], &str, &str); 6] = [
        (
            &["ask"],
            "zh_CN.UTF-8",
            "miyu: 缺少消息\n提示：用法：miyu ask \"<消息>\"",
        ),
        // 嵌着的子命令少了必写的照帮助页说怎么写（施工 K-7 修）。
        (
            &["kb", "read", "notes"],
            "zh_CN.UTF-8",
            "miyu: 缺少参数\n提示：用法：miyu kb read <库> <文档>",
        ),
        (
            &["memory", "edit", "m5"],
            "C",
            "miyu: missing arguments\nhint: usage: miyu memory edit <id> [<text>]",
        ),
        (
            &["ask", "--format", "xml", "hi"],
            "C",
            "miyu: invalid value 'xml' for '--format'\nhint: possible values: text, json or jsonl",
        ),
        // 短写的 `-s`、`-c` 就是 `--session`、`--continue`。
        (
            &["ask", "-s", "x", "-c", "hi"],
            "zh_CN.UTF-8",
            "miyu: '--session' 不能和 '--continue' 同时使用",
        ),
        (
            &["undo", "-s"],
            "zh_CN.UTF-8",
            "miyu: 选项 '--session' 需要一个值",
        ),
    ];
    for (args, lang, said) in cases {
        let output = miyu(&home, lang, args);
        assert_eq!(output.status.code(), Some(2), "{args:?}：{output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            format!("{said}\n"),
            "{args:?}"
        );
        assert!(output.stdout.is_empty(), "{output:?}");
    }
}

#[test]
fn a_nested_command_mistake_is_one_error_with_hints_and_exit_code_2() {
    let home = Home::new();
    let cases: [(&[&str], &str, &str); 2] = [
        (
            &["service", "frob"],
            "C",
            "miyu: unknown command 'frob' for 'miyu service'\nhint: see 'miyu service --help'",
        ),
        (
            &["session", "export", "--for", "read"],
            "C",
            "miyu: missing option '--md'",
        ),
    ];
    for (args, lang, said) in cases {
        let output = miyu(&home, lang, args);
        assert_eq!(output.status.code(), Some(2), "{args:?}：{output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            format!("{said}\n"),
            "{args:?}"
        );
        assert!(output.stdout.is_empty(), "{output:?}");
    }
}

#[test]
fn the_program_calls_itself_miyu_whatever_its_file_is_called() {
    // Windows 上可执行文件叫 `miyu.exe`，clap 默认照文件名说话，会说成「miyu.exe service …」（施工 5-8 在 CI 上查出来的）。
    // 换个名字跑，这台机器上也照得出来。换名字用硬链接，放在主程序旁边（test-ext 补）：拷的时候开着写的句柄，别的测试这时
    // 拉起的子进程在 exec 以前也开着它，接着跑刚拷好的这个，Linux 回 `ETXTBSY`。临时目录可能和主程序不在一个文件系统上
    // （tmpfs），连不成，所以放主程序旁边；连不成的才拷。
    let home = Home::new();
    let renamed = Renamed(std::path::Path::new(MIYU).with_file_name(format!(
        "renamed-miyu-{}{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    )));
    let renamed = &renamed.0;
    if std::fs::hard_link(MIYU, renamed).is_err() {
        std::fs::copy(MIYU, renamed).expect("拷得了主程序");
    }
    let output = Command::new(renamed)
        .args(["service", "frob"])
        .env("MIYU_HOME", home.root.path())
        .envs(crate::support::offline(home.root.path()))
        .env("LANG", "C")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .output()
        .expect("跑得起来");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "miyu: unknown command 'frob' for 'miyu service'\nhint: see 'miyu service --help'\n"
    );
}

/// 换了名字的主程序：用完删掉。
struct Renamed(std::path::PathBuf);

impl Drop for Renamed {
    fn drop(&mut self) {
        drop(std::fs::remove_file(&self.0));
    }
}
