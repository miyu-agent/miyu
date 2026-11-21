//! 主程序 `miyu`（`internal/designs/12-进程形态与分发.md` 第三节，施工 3-9）：一个程序，像 busybox 那样按子命令
//! 分发。`miyu ask` 是最薄的头（施工 3-9 下）；`miyu undo`（`miyu rewind`）、`miyu restore` 撤掉最后一轮、恢复（施工 4-7 下，改名施工 4-7 补）；
//! `miyu redo` 重做最后一轮（施工 4-7 再补）；
//! `miyu compact` 手动压缩（施工 6-8）；`miyu recap` 一句话回顾（施工 3-8 四补）；`miyu rename` 给会话起名（施工 3-8 五补）；`miyu config` 看配置（施工 8-2）；`miyu provider` 管供应商的 key（施工 8-5）；`miyu setup` 接上第一个模型（施工 8-11）；`miyu rescue` 拉起 LiteMiyu 的应急模式（施工 RC-1）；`miyu core` 是核心进程，由头拉起，
//! 不写进帮助。
//!
//! 不认识的子命令就报错，退出码 2，绝不当成对话发给核心（R4，`22-命令行.md` 第二节）。帮助页、参数写错时说的
//! 那一句都是自己写的，跟着界面语言（施工 4-11，`internal/blueprint/cli/main.md`）。

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::error::ErrorKind;
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};

use miyu_cli::help::{Page, page};
use miyu_cli::language::{self, Language};
use miyu_cli::{Direction, misuse_offering};

/// 退出码：用法不对（`22-命令行.md` 第二节）。
const USAGE: u8 = 2;

/// 主程序的参数。名字定死成 `miyu`：clap 默认照可执行文件的名字，Windows 上会在用法和报错里带出 `miyu.exe`（施工 5-8
/// 查出来的）。
#[derive(Parser)]
#[command(name = "miyu", bin_name = "miyu", version, propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

/// 子命令。
#[derive(Subcommand)]
enum Command {
    /// 说一句话，打印她的回答。
    Ask(miyu_cli::Ask),
    /// 撤掉当前会话的最后一轮（文件不改回，施工 U-1）。也可以写成 `rewind`（施工 4-7 补）。
    #[command(alias = "rewind")]
    Undo(miyu_cli::Undo),
    /// 发下一句之前，恢复最近一次撤销（原来叫 `redo`，施工 4-7 补改名）。
    Restore(miyu_cli::Undo),
    /// 撤掉最后一轮，把人那句话（或者换成写的话）再发一次，让她重新做（施工 4-7 再补）。
    Redo(miyu_cli::Redo),
    /// 把当前会话的上下文压缩成摘要，可以附上要求（施工 6-8）。
    Compact(miyu_cli::Compact),
    /// 一句话回顾当前会话：在做什么、做完了什么、卡在哪（施工 3-8 四补）。
    Recap(miyu_cli::Recap),
    /// 给当前会话起名（施工 3-8 五补）。
    Rename(miyu_cli::Rename),
    /// 登录要浏览器授权的网址 MCP 服务（施工 X-42 上）。
    Mcp(miyu_cli::Mcp),
    /// 看总线上谁提供了什么、可不可用（施工 BUS-3）。
    Bus(miyu_cli::Bus),
    /// 让核心开机、登录就起：装、卸、看本人这一层的服务（施工 Z-1 下）。
    Service(miyu_cli::Service),
    /// 升级 Miyu 本身，或者说该用哪个包管理器（施工 RL-4）。
    Upgrade(miyu_cli::Upgrade),
    /// 核心坏了时进 LiteMiyu 的应急模式修：不经过核心，照 Miyu 的对话模型填好环境变量，拉起 `litemiyu --rescue`（施工 RC-1）。
    Rescue(miyu_cli::Rescue),
    /// 看配置、改配置、信任项目配置（施工 8-2、8-3）。
    Config(miyu_cli::Config),
    /// 查手写的文件有没有写错：配置、密钥文件、人格（施工 8-30）。
    Check(miyu_cli::Check),
    /// 看她记了你什么，搜、记、改、忘、清空（施工 R-3 再补）。
    Memory(miyu_cli::Memory),
    /// 看装了哪些软件包，装、卸（施工 T-3）。
    Pkg(miyu_cli::Pkg),
    /// 管知识库：列、搜、读、建、改、加、删、挂、重建（施工 K-7 下）。
    Kb(miyu_cli::Kb),
    /// 给写包的人用的：查模型驱动的编码稳不稳（施工 MD-3）。
    Dev(miyu_cli::Dev),
    /// 管定时任务：列、建、删、停、续（施工 J-3 下）。
    Schedule(miyu_cli::Schedule),
    /// 导出、导入会话的原始包（施工 SE-1 中上）。
    Session(miyu_cli::Session),
    /// 供应商的 key：存、列、删（施工 8-5，施工 Y-13 上从 `login`、`logout` 挪来）。
    Provider(miyu_cli::Provider),
    /// 第一次接入模型（施工 8-11）。
    Setup(miyu_cli::Setup),
    /// 核心进程：不带子命令的是跑核心本身，由头拉起，不用人敲。`start`、`status`、`stop` 管在后台跑的那一个（施工 CL-4、
    /// CL-8，2026-11-18 项目主人定：`miyu core start | status | stop` 只管核心）。
    Core {
        /// 空闲多少秒以后退出。
        #[arg(long, hide = true)]
        idle_seconds: Option<u64>,
        /// 不写的是跑核心；写了的管在跑的那一个。
        #[command(subcommand)]
        action: Option<CoreAction>,
    },
}

/// `miyu core` 下面的动作（施工 CL-4、CL-8）。
#[derive(Debug, Clone, Copy, Subcommand)]
enum CoreAction {
    /// 没在跑就拉起来，等它好了再回；只拉核心，不开界面。
    Start,
    /// 在不在跑、哪一版、数据根在哪；只看，不拉起。
    Status {
        /// `text` 给人看（默认），`json` 给头、脚本。
        #[arg(long, value_enum, default_value_t = miyu_cli::Format::Text)]
        format: miyu_cli::Format,
    },
    /// 叫在跑的核心停下，等它退了再回；没在跑的什么都不做。
    Stop,
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    // 头拉起核心的快路（施工 LW-8，`core.md`「起来的先后」）：认得出的直接进核心，不搭整棵命令行树。
    if let Some(idle) = running_core(args.get(1..).unwrap_or_default()) {
        return miyu_core::main(miyu_core::Options {
            idle: idle.map(Duration::from_secs),
        });
    }
    dispatch(args)
}

/// 前台跑核心的那几种写法：`core`、`core --idle-seconds <秒>`、`core --idle-seconds=<秒>`，认得出的交回空闲秒数（没写的
/// 是 `Some(None)`）。别的一律交回 `None`，照整棵命令行树解析：管理用的子命令、要帮助的、写错的、不是 UTF-8 的，报错的字
/// 都由 clap 照原样说（施工 LW-8）。秒数照 clap 解析 `u64` 的同一个 `str::parse`，认不出的也交给 clap 报。
///
/// 为什么要这条路：核心进程只认这一种写法，原来也要先搭整棵命令行树（`Cli::command()` 两次、各子命令的参数表、整套解析），
/// 一共 266 kB 的代码执行一遍就留在核心的内存里（蓝图 `perf.md`「排函数」：留在内存里的约等于执行过的）。
fn running_core(words: &[OsString]) -> Option<Option<u64>> {
    let words: Vec<&str> = words
        .iter()
        .map(|word| word.to_str())
        .collect::<Option<_>>()?;
    match words.as_slice() {
        ["core"] => Some(None),
        ["core", "--idle-seconds", seconds] => seconds.parse().ok().map(Some),
        ["core", word] => word.strip_prefix("--idle-seconds=")?.parse().ok().map(Some),
        _ => None,
    }
}

/// 照整棵命令行树解析、分发：快路（[`running_core`]）认不出的都走这里。
// 单拎成一个不内联的函数（施工 LW-8）：并进 `main` 的话，核心进程走快路也算执行过整个 `main`，这一大块跟着留在内存里。
#[inline(never)]
fn dispatch(args: Vec<OsString>) -> ExitCode {
    let language = language::current();
    // 软件包加的子命令（施工 9-2）：第一个词不是内置的（不认识的、`help`、选项）才照清单找，`miyu core`、`miyu ask` 不多读盘。
    // 脚本调工具（施工 M-3，设计 39）：不读清单、不连核心的协议，经命令自己的端点调。
    if args.get(1).is_some_and(|first| first == "call") {
        return miyu_cli::call(&args[2..], language);
    }
    let builtins = builtins();
    let first = args.get(1).and_then(|first| first.to_str());
    let added = match first {
        Some(first) if first != "help" && builtins.iter().any(|name| name == first) => Vec::new(),
        _ => {
            // 旧版的数据根上找不到包加的命令（施工 FH-3下）：先说是旧版的，别当成不认识的命令。
            if first.is_some_and(|word| word != "help" && !word.starts_with('-'))
                && let Some(code) = miyu_cli::packages::old_layout(language, &mut std::io::stderr())
            {
                return ExitCode::from(code);
            }
            let names: Vec<&str> = builtins.iter().map(String::as_str).collect();
            miyu_cli::packages::installed(&names)
        }
    };
    let main = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("miyu"));
    if let Some(code) =
        miyu_cli::packages::forward(&args, &added, &main, language, &mut std::io::stderr())
    {
        return ExitCode::from(code);
    }
    let section = miyu_cli::packages::help_section(language, &added);
    let help = miyu_cli::packages::with_section(page(language, Page::Miyu), &section);
    let command = dressed(Cli::command(), language, help.clone());
    let cli = match command
        .try_get_matches()
        .and_then(|matches| Cli::from_arg_matches(&matches))
    {
        Ok(cli) => cli,
        Err(error) => return refused(error, language),
    };
    match cli.command {
        Some(Command::Ask(args)) => miyu_cli::ask(args, core),
        Some(Command::Undo(args)) => miyu_cli::undo(args, Direction::Undo, core),
        Some(Command::Restore(args)) => miyu_cli::undo(args, Direction::Restore, core),
        Some(Command::Redo(args)) => miyu_cli::redo(args, core),
        Some(Command::Compact(args)) => miyu_cli::compact(args, core),
        Some(Command::Recap(args)) => miyu_cli::recap(args, core),
        Some(Command::Rename(args)) => miyu_cli::rename(args, core),
        Some(Command::Mcp(args)) => miyu_cli::mcp(args, core),
        Some(Command::Bus(args)) => miyu_cli::bus(args, core),
        Some(Command::Service(args)) => miyu_cli::service(args),
        Some(Command::Upgrade(args)) => miyu_cli::upgrade(args),
        // 不连核心、不拉起核心（施工 RC-1）：对话模型照磁盘读，`miyu_core::rescue` 只借读模型的那一套。
        Some(Command::Rescue(args)) => miyu_cli::rescue(args, miyu_core::rescue::model),
        // 不带子命令、在终端里：打开界面的设置页（施工 9-3，原来的 8-24）；界面不认设置页的照旧印帮助。
        Some(Command::Config(args)) if args.command.is_none() && terminal() => {
            match miyu_cli::head::open(Some("config"), core) {
                miyu_cli::head::Opened::Ran(code) => ExitCode::from(code),
                miyu_cli::head::Opened::NoPage => miyu_cli::config(args, core),
            }
        }
        Some(Command::Config(args)) => miyu_cli::config(args, core),
        Some(Command::Check(args)) => miyu_cli::check(args, core),
        Some(Command::Memory(args)) => miyu_cli::memory(args, core),
        Some(Command::Pkg(args)) => miyu_cli::pkg(args, core),
        Some(Command::Kb(args)) => miyu_cli::kb(args, core),
        Some(Command::Dev(args)) => miyu_cli::dev(args, core),
        Some(Command::Schedule(args)) => miyu_cli::schedule(args, core),
        Some(Command::Session(args)) => miyu_cli::session(args, core),
        Some(Command::Provider(args)) => miyu_cli::login(args.into(), core),
        Some(Command::Setup(args)) => miyu_cli::setup(args, core),
        Some(Command::Core {
            action: Some(CoreAction::Start),
            ..
        }) => miyu_cli::core_start(core),
        Some(Command::Core {
            action: Some(CoreAction::Status { format }),
            ..
        }) => miyu_cli::core_status(format),
        Some(Command::Core {
            action: Some(CoreAction::Stop),
            ..
        }) => miyu_cli::core_stop(),
        Some(Command::Core { idle_seconds, .. }) => miyu_core::main(miyu_core::Options {
            idle: idle_seconds.map(Duration::from_secs),
        }),
        // 不带子命令（施工 9-3）：在终端里打开 `ui.head` 指的界面；不在终端里（被脚本调、接管道）印帮助、退出码 2。
        None if terminal() => match miyu_cli::head::open(None, core) {
            miyu_cli::head::Opened::Ran(code) => ExitCode::from(code),
            miyu_cli::head::Opened::NoPage => ExitCode::from(USAGE),
        },
        None => {
            print!("{help}");
            ExitCode::from(USAGE)
        }
    }
}

/// 哪个子命令印哪一页（施工 4-11），连同它下面每一层的子命令（施工 HLP-1）。`rewind` 是 `undo` 的别名，不另算。
const PAGES: [(&str, Page); 23] = [
    ("ask", Page::Ask),
    ("undo", Page::Undo),
    ("restore", Page::Restore),
    ("redo", Page::Redo),
    ("compact", Page::Compact),
    ("recap", Page::Recap),
    ("rename", Page::Rename),
    ("provider", Page::Provider),
    ("setup", Page::Setup),
    ("check", Page::Check),
    ("config", Page::Config),
    ("memory", Page::Memory),
    ("pkg", Page::Pkg),
    ("dev", Page::Dev),
    ("bus", Page::Bus),
    ("kb", Page::Kb),
    ("schedule", Page::Schedule),
    ("session", Page::Session),
    ("mcp", Page::Mcp),
    ("service", Page::Service),
    ("upgrade", Page::Upgrade),
    ("rescue", Page::Rescue),
    ("core", Page::Core),
];

/// 帮助页换成自己写的（施工 4-11）：`-h`、`--help`、`miyu help <子命令>` 都印它们，clap 生成的一个字都不印。
fn dressed(command: clap::Command, language: Language, help: String) -> clap::Command {
    PAGES
        .into_iter()
        .fold(command.override_help(help), |command, (name, which)| {
            command.mut_subcommand(name, |sub| everywhere(sub, page(language, which)))
        })
}

/// 这个命令和它下面每一层的子命令都印 `help` 这一页（施工 HLP-1）：原来照名单一个个挂，后加的子命令漏了就印 clap 照源码
/// 注释生成的那一页。
fn everywhere(command: clap::Command, help: &'static str) -> clap::Command {
    let names: Vec<String> = command
        .get_subcommands()
        .map(|sub| sub.get_name().to_owned())
        .collect();
    names
        .into_iter()
        .fold(command.override_help(help), |command, name| {
            command.mut_subcommand(name, |sub| everywhere(sub, help))
        })
}

/// 标准输入、标准输出都是终端（施工 9-3）：只有这时才拉起界面。
fn terminal() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

/// 内置的子命令（施工 9-2）：每一个的名字、别名，加上 `help`。软件包的子命令撞了它们的不转交、不列。
fn builtins() -> Vec<String> {
    let command = Cli::command();
    command
        .get_subcommands()
        .flat_map(|sub| {
            std::iter::once(sub.get_name().to_string())
                .chain(sub.get_all_aliases().map(str::to_string))
        })
        .chain(std::iter::once("help".to_string()))
        .collect()
}

/// 拉起核心的命令：自己这个程序，加上 `core`。
fn core() -> std::process::Command {
    let mut core = std::process::Command::new(
        std::env::current_exe().unwrap_or_else(|_| PathBuf::from("miyu")),
    );
    core.arg("core");
    core
}

/// 参数不对：帮助、版本照常印在标准输出上，退出码 0；别的在标准错误上照设计 47 说（`miyu: ` 打头，能照着改的另起
/// `hint:`），退出码 2。子命令后面带 `--version`、`-V` 的（施工 VS-1）照主程序的印 `miyu <版本>`：clap 原样印成 `miyu-ask <版本>`。
#[expect(
    clippy::let_underscore_must_use,
    reason = "帮助印不出来，也没有别处可说了"
)]
fn refused(error: clap::Error, language: Language) -> ExitCode {
    match error.kind() {
        ErrorKind::DisplayHelp => {
            let _ = error.print();
            ExitCode::SUCCESS
        }
        ErrorKind::DisplayVersion => {
            print!("{}", Cli::command().render_version());
            ExitCode::SUCCESS
        }
        _ => {
            for line in misuse_offering(&error, language, &miyu_cli::packages::offering) {
                eprintln!("{line}");
            }
            ExitCode::from(USAGE)
        }
    }
}

#[cfg(test)]
mod tests {
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
}
