//! 参数写错时说的那一句（施工 4-11）：每一种错、两种语言；值怎么连；控制字符换掉；必写的只有一个。

use clap::{Args, Command};

use super::{listed, misuse, misuse_offering, synopsis};
use crate::help::Page;
use crate::language::Language;
use crate::{Ask, Config, Kb, Memory, Pkg, Sandbox, Undo};

/// 和主程序一样的几条子命令。
fn miyu() -> Command {
    Command::new("miyu")
        .subcommand(Ask::augment_args(Command::new("ask")))
        .subcommand(Undo::augment_args(Command::new("undo")))
        .subcommand(Sandbox::augment_args(Command::new("sandbox")))
        .subcommand(Memory::augment_args(Command::new("memory")))
        .subcommand(Kb::augment_args(Command::new("kb")))
}

/// 敲 `miyu <args>`：clap 报的错说成的那几行，连成一段。
fn said(args: &[&str], language: Language) -> String {
    let error = miyu()
        .try_get_matches_from(std::iter::once("miyu").chain(args.iter().copied()))
        .expect_err("这样敲是错的");
    misuse(&error, language).join("\n")
}

#[test]
fn each_kind_of_mistake_has_one_sentence() {
    let cases: [(&[&str], &str, &str); 8] = [
        (
            &["ask", "--bogus", "hi"],
            "miyu: 无法识别的选项 '--bogus'",
            "miyu: unrecognized option '--bogus'",
        ),
        (
            &["ask", "-x", "hi"],
            "miyu: 无法识别的选项 '-x'",
            "miyu: unrecognized option '-x'",
        ),
        (
            &["--bogus"],
            "miyu: 无法识别的选项 '--bogus'",
            "miyu: unrecognized option '--bogus'",
        ),
        (
            &["undo", "extra"],
            "miyu: 多余的参数 'extra'",
            "miyu: unexpected argument 'extra'",
        ),
        (
            &["ask", "-s", "x", "-c", "hi"],
            "miyu: '--session' 不能和 '--continue' 同时使用",
            "miyu: '--session' cannot be used with '--continue'",
        ),
        (
            &["undo", "--session"],
            "miyu: 选项 '--session' 需要一个值",
            "miyu: option '--session' requires a value",
        ),
        (
            &["ask", "--format"],
            "miyu: 选项 '--format' 需要一个值",
            "miyu: option '--format' requires a value",
        ),
        (
            &["ask", "--format", "xml", "hi"],
            "miyu: '--format' 的值无效：'xml'\n提示：可选的值：text、json 或 jsonl",
            "miyu: invalid value 'xml' for '--format'\nhint: possible values: text, json or jsonl",
        ),
    ];
    for (args, chinese, english) in cases {
        assert_eq!(said(args, Language::Chinese), chinese, "{args:?}");
        assert_eq!(said(args, Language::English), english, "{args:?}");
    }
    // 不认识的子命令照 22 第二节的那一句。
    assert_eq!(
        said(&["hello"], Language::Chinese),
        "miyu: 未知命令 'hello'\n提示：运行 'miyu --help' 查看可用命令"
    );
}

#[test]
fn anything_else_says_what_clap_said_on_its_first_line() {
    assert_eq!(
        said(&["ask", "--continue=yes", "hi"], Language::Chinese),
        "miyu: 参数错误：unexpected value 'yes' for '--continue' found; no more were expected"
    );
    assert_eq!(
        said(&["ask", "--continue=yes", "hi"], Language::English),
        "miyu: unexpected value 'yes' for '--continue' found; no more were expected"
    );
}

#[test]
fn control_characters_typed_in_are_replaced() {
    assert_eq!(
        said(&["ask", "--bo\x1b[2Jgus", "hi"], Language::Chinese),
        "miyu: 无法识别的选项 '--bo\u{FFFD}[2Jgus'"
    );
    assert_eq!(
        said(&["undo", "a\x07b"], Language::English),
        "miyu: unexpected argument 'a\u{FFFD}b'"
    );
}

#[test]
fn values_are_joined_like_a_sentence() {
    let values = |names: &[&str]| names.iter().map(ToString::to_string).collect::<Vec<_>>();
    assert_eq!(listed(&values(&[]), "、", " 或 "), "");
    assert_eq!(listed(&values(&["a"]), "、", " 或 "), "a");
    assert_eq!(listed(&values(&["a", "b"]), "、", " 或 "), "a 或 b");
    assert_eq!(listed(&values(&["a", "b", "c"]), "、", " 或 "), "a、b 或 c");
    assert_eq!(listed(&values(&["a", "b", "c"]), ", ", " or "), "a, b or c");
}

#[test]
fn nothing_is_required_at_the_top() {
    // 施工 AP-2 起 `ask` 的要说的话也可以不写（从标准输入读）：最外面那一层一个必写的都没有。多了一个，「少了要说的话」那一句
    // 就说错了：这里红了，`misuse` 跟着改。
    let required: Vec<String> = miyu()
        .get_subcommands()
        .flat_map(|command| {
            command
                .get_arguments()
                .filter(|arg| arg.is_required_set())
                .map(|arg| format!("{} {}", command.get_name(), arg.get_id()))
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(required.is_empty(), "{required:?}");
}

#[test]
fn nested_commands_have_their_own_sentences() {
    // `miyu sandbox` 是第一条嵌着子命令的（施工 5-8）：少了、写错了，说的是它那一层，不提 `miyu ask`。
    let cases: [(&[&str], &str, &str); 4] = [
        (
            &["sandbox"],
            "miyu: 'miyu sandbox' 需要子命令\n提示：可用的有：setup 或 remove",
            "miyu: 'miyu sandbox' requires a subcommand\nhint: one of: setup or remove",
        ),
        (
            &["sandbox", "frob"],
            "miyu: 'miyu sandbox' 没有子命令 'frob'\n提示：运行 'miyu sandbox --help' 查看可用命令",
            "miyu: unknown command 'frob' for 'miyu sandbox'\nhint: see 'miyu sandbox --help'",
        ),
        (
            &["sandbox", "setup", "--owner-home", "/x"],
            "miyu: 缺少选项 '--owner-sid'",
            "miyu: missing option '--owner-sid'",
        ),
        (
            &["sandbox", "remove", "extra"],
            "miyu: 多余的参数 'extra'",
            "miyu: unexpected argument 'extra'",
        ),
    ];
    for (args, chinese, english) in cases {
        assert_eq!(said(args, Language::Chinese), chinese, "{args:?}");
        assert_eq!(said(args, Language::English), english, "{args:?}");
    }
    // 最外面那一层写错的，照旧那一句。
    assert_eq!(
        said(&["sandboxx"], Language::English),
        "miyu: unknown command 'sandboxx'\nhint: see 'miyu --help'"
    );
}

/// `--add-dir` 后面不是一个已经有的目录（施工 5-10 上）：照写的原样说是哪一个。
#[test]
fn an_added_dir_that_is_not_there_has_its_sentence() {
    let args = ["ask", "--add-dir", "no-such-dir-for-miyu-cli-tests", "hi"];
    assert_eq!(
        said(&args, Language::Chinese),
        "miyu: '--add-dir' 的值无效：'no-such-dir-for-miyu-cli-tests' 不是已有的目录"
    );
    assert_eq!(
        said(&args, Language::English),
        "miyu: invalid value for '--add-dir': 'no-such-dir-for-miyu-cli-tests' is not an existing directory"
    );
}

/// `--timeout` 后面不是一个时长（施工 7-9）：照写的原样说是哪一个，举几个能写的样子。
#[test]
fn a_timeout_that_is_not_a_duration_has_its_sentence() {
    let args = ["ask", "--timeout", "5min", "hi"];
    assert_eq!(
        said(&args, Language::Chinese),
        "miyu: '--timeout' 的值无效：'5min'\n提示：写成 30s、10m、1h 这样的时长"
    );
    assert_eq!(
        said(&args, Language::English),
        "miyu: invalid value for '--timeout': '5min'\nhint: use a duration such as 30s, 10m or 1h"
    );
}

/// `--from` 后面是空的、只有空白（施工 7-10）：说要写别的 harness 的名字；后面什么都没写的照「少了值」说。
#[test]
fn a_blank_from_has_its_sentence() {
    for value in ["", "  "] {
        let args = ["ask", "--from", value, "hi"];
        assert_eq!(
            said(&args, Language::Chinese),
            "miyu: 选项 '--from' 需要另一个 harness 的名字"
        );
        assert_eq!(
            said(&args, Language::English),
            "miyu: option '--from' requires the name of the other harness"
        );
    }
    assert_eq!(
        said(&["ask", "hi", "--from"], Language::Chinese),
        "miyu: 选项 '--from' 需要一个值"
    );
}

#[test]
fn a_missing_word_of_a_nested_command_shows_how_it_is_written() {
    // 施工 K-7 修（验收手册待定 1）：嵌着的子命令少了必写的，照这个命令的帮助页说它怎么写，同 `ask` 那一句；原来一律说成
    // `ask` 的那一句。
    let cases: [(&[&str], &str, &str); 3] = [
        (
            &["kb", "read", "notes"],
            "miyu: 缺少参数\n提示：用法：miyu kb read <库> <文档>",
            "miyu: missing arguments\nhint: usage: miyu kb read <kb> <document>",
        ),
        (
            &["memory", "forget"],
            "miyu: 缺少参数\n提示：用法：miyu memory forget <编号>",
            "miyu: missing arguments\nhint: usage: miyu memory forget <id>",
        ),
        (
            &["kb", "search"],
            "miyu: 缺少参数\n提示：用法：miyu kb search <词>",
            "miyu: missing arguments\nhint: usage: miyu kb search <words>",
        ),
    ];
    for (args, chinese, english) in cases {
        assert_eq!(said(args, Language::Chinese), chinese, "{args:?}");
        assert_eq!(said(args, Language::English), english, "{args:?}");
    }
}

#[test]
fn every_nested_command_with_required_words_is_on_its_help_page() {
    // 嵌着的子命令有必写的，帮助页的「命令」那一节就得有它那一行：少了参数时照它说（`synopsis`）。
    let tops: [(Command, Page); 4] = [
        (Memory::augment_args(Command::new("memory")), Page::Memory),
        (Kb::augment_args(Command::new("kb")), Page::Kb),
        (Pkg::augment_args(Command::new("pkg")), Page::Pkg),
        (Config::augment_args(Command::new("config")), Page::Config),
    ];
    for (top, page) in tops {
        for sub in top.get_subcommands() {
            if !sub.get_arguments().any(|arg| arg.is_required_set()) {
                continue;
            }
            for language in [Language::Chinese, Language::English] {
                assert!(
                    synopsis(language, page, sub.get_name()).is_some(),
                    "{} {} 不在帮助页上",
                    top.get_name(),
                    sub.get_name()
                );
            }
        }
    }
}

/// 不认识的命令是一个没装的软件包带的（施工 CL-7）：提示去装它；查不到的照旧。
#[test]
fn a_command_from_a_package_not_installed_says_which_package() {
    let error = miyu()
        .try_get_matches_from(["miyu", "web"])
        .expect_err("没有这个命令");
    let offered = |name: &str| (name == "web").then(|| "miyu-webui".to_string());
    assert_eq!(
        misuse_offering(&error, Language::Chinese, &offered).join("\n"),
        "miyu: 未知命令 'web'\n提示：软件包 miyu-webui 提供这个命令，用 'miyu pkg install miyu-webui' 安装"
    );
    assert_eq!(
        misuse_offering(&error, Language::English, &offered).join("\n"),
        "miyu: unknown command 'web'\nhint: package miyu-webui provides it; install it with 'miyu pkg install miyu-webui'"
    );
    let none = |_: &str| None;
    assert_eq!(
        misuse_offering(&error, Language::Chinese, &none).join("\n"),
        "miyu: 未知命令 'web'\n提示：运行 'miyu --help' 查看可用命令"
    );
}
