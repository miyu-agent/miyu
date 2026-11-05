//! 软件包加的子命令（施工 9-2，`docs/blueprint/packages.md`「转交」，`cli/main.md`「分发」）：`miyu <名字> …` 不是内置的
//! 子命令时，照两层的清单找 `[command]` 是它的那个包，原样跑包里的程序：参数、环境、标准输入输出照原样，退出码照它的；环境
//! 多它在各层的那几处（设计 55 第四节，`miyu_store::program::PackageDirs`：包的文件夹、配置、状态、缓存、日志、暂存、运行时，
//! 跑之前建好、清掉暂存），同核心拉起扩展。
//! `miyu help <名字>` 转成 `<程序> --help`：帮助由包自己说。`miyu -h` 多一节「软件包加的命令」。
//!
//! 和内置的撞名：内置的优先，不转交、不列（装包时拦随装包那一步）。清单照磁盘现读，不连核心：转交不该为了找一个程序拉起
//! 核心。

mod offered;
#[cfg(test)]
mod tests;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::run;

pub use offered::offering;

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use miyu_config::phrases::Phrases;
use miyu_store::env::Env;
use miyu_store::packages::{Launch, Packages, locate};
use miyu_store::program::PackageDirs;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::language::Language;

/// 帮助页里命令那一列多宽，不含缩进的两格（`help/<语言>/miyu.txt`：缩进两格，名字连空白到第 22 列，说明从第 22 列开始；施工 AP-1）。
const NAME_COLUMN: usize = 20;

/// 一个包加的子命令。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added {
    /// `miyu` 后面敲的那个词。
    pub name: String,
    /// 跑哪个程序：程序名，不带路径。
    pub program: String,
    /// 转交时加在人敲的参数前面的（施工 F-13）：没写的是空的。
    pub args: Vec<String>,
    /// 一句说明，照语言挑。
    pub about: Phrases,
    /// 清单在哪：没装程序时说给人听。
    pub manifest: PathBuf,
    /// 读到这份清单时的资源目录：转交时设成 `MIYU_RESOURCES`（施工 S-6 中查出来的）。
    pub resources: PathBuf,
    /// 哪个包：转交时照它在数据根里建好它的几处（施工 F-15、FH-3下）。
    pub package: String,
    /// 读到这份清单时的数据根。
    pub home: PathBuf,
}

/// 两层里读成了的、带 `[command]` 的清单加的子命令，照编号排，一个包加几个的照写的先后（施工 F-13）。写错的、子命令名被先读到的占了的（`command_taken`）不算；
/// 名字是内置子命令 `builtins` 里的也不算：内置的优先。数据根、资源目录找不到的当没有。
pub fn added(env: &Env, builtins: &[&str]) -> Vec<Added> {
    let (Ok(root), Ok(resources)) = (DataRoot::locate(env), ResourceRoot::locate(env)) else {
        return Vec::new();
    };
    let places = Packages::system(&resources, &root);
    places
        .read()
        .into_iter()
        .filter_map(|found| Some((found.read.ok()?, found.path, found.id)))
        .flat_map(|(manifest, path, package)| {
            let resources = resources.path().to_path_buf();
            let home = root.path().to_path_buf();
            manifest
                .commands
                .into_iter()
                .filter(|command| !builtins.contains(&command.name.as_str()))
                .map(move |command| Added {
                    name: command.name,
                    program: command.program,
                    args: command.args,
                    about: command.about,
                    manifest: path.clone(),
                    resources: resources.clone(),
                    package: package.clone(),
                    home: home.clone(),
                })
        })
        .collect()
}

/// 同 [`added`]，照这个进程的环境找数据根、资源目录。
pub fn installed(builtins: &[&str]) -> Vec<Added> {
    added(&Env::current(), builtins)
}

/// `miyu -h` 多的那一节：没有的是空的。名字照帮助页命令那一列对齐，说明照界面语言挑（挑法同 `package.list`）。
pub fn help_section(language: Language, added: &[Added]) -> String {
    if added.is_empty() {
        return String::new();
    }
    let title = match language {
        Language::Chinese => "软件包加的命令：",
        Language::English => "Commands from packages:",
    };
    let mut section = format!("{title}\n");
    for one in added {
        // 子命令名只有小写字母、数字、`-`（清单查过），一个字一列。
        let pad = NAME_COLUMN.saturating_sub(one.name.len()).max(2);
        let about = pick(&one.about, language).unwrap_or_default();
        section.push_str(&format!("  {}{}{about}\n", one.name, " ".repeat(pad)));
    }
    section
}

/// 帮助页 `page` 里，接上 `section`（空的照原样）：接在「选项：」（英文 `Options:`）那一节前面，也就是最后一组命令后面，空一行隔开
/// （施工 AP-1）。找不到这一节的，照原来的办法：页面是用法一节、命令一节、再往下的几节，节和节之间空一行，接在第二个空行那里。
pub fn with_section(page: &str, section: &str) -> String {
    if section.is_empty() {
        return page.to_string();
    }
    let options = ["\n\n选项：\n", "\n\nOptions:\n"]
        .iter()
        .filter_map(|mark| page.find(mark))
        .min();
    let end = options.or_else(|| {
        let start = page.find("\n\n").map(|at| at + 2)?;
        page[start..].find("\n\n").map(|at| start + at)
    });
    match end {
        Some(at) => format!("{}\n\n{}{}", &page[..at], section.trim_end(), &page[at..]),
        None => format!("{page}\n{section}"),
    }
}

/// 照语言挑一句：这种语言、`en`、`zh`、`ja` 的先后。
fn pick(phrases: &Phrases, language: Language) -> Option<String> {
    [language.code(), "en", "zh", "ja"]
        .iter()
        .find_map(|code| phrases.get(*code).cloned())
}

/// 第一个词不是内置的子命令（照清单找包加的命令之前）：数据根是旧版 Miyu 的（只有旧标记 `.miyu-root`，设计 55 第六节第 1 条），
/// 读不出包的清单，原来会当成不认识的命令报「未知命令」（webui 实测 FH-3上 时撞见）。这里先认布局，说同一句「旧版的数据根」、
/// 交回退出码 1；别的（新布局、空的、找不到数据根）交回没有，照常往下走。数据根照这个进程的环境找。
pub fn old_layout(language: Language, err: &mut dyn Write) -> Option<u8> {
    let root = DataRoot::locate(&Env::current()).ok()?;
    let at = |name: &str| std::fs::symlink_metadata(root.path().join(name)).is_ok();
    if !at(miyu_store::root::OLD_MARKER) || at(miyu_store::root::MARKER) {
        return None;
    }
    let said = miyu_store::root::PrepareError::OldLayout(root.path().to_path_buf()).to_string();
    crate::diagnostic::error(err, language, &crate::diagnostic::Problem::new(said));
    Some(1)
}

/// 该转交的交回退出码，不该的（第一个词是内置的、是选项、清单里没有）交回没有，照常解析。`args` 是整个命令行（第 0 个是
/// 主程序），主程序在 `main`；没装程序、跑不起来的照 `language` 在 `err` 上说。
pub fn forward(
    args: &[OsString],
    added: &[Added],
    main: &Path,
    language: Language,
    err: &mut dyn Write,
) -> Option<u8> {
    let (one, rest) = target(args, added)?;
    // 包文件夹就是清单所在的那一层（施工 F-8 上再补：程序先找它的 `bin/`）。
    let dir = one.manifest.parent().unwrap_or(Path::new(""));
    let Some(launch) = locate(&one.program, dir, main) else {
        say(err, &not_installed(one, language));
        return Some(1);
    };
    // 先照常建数据根（施工 CL-7 修）：标记、骨架都在了再建包的状态目录。不先建的，空的数据根里只多出一个
    // `state/packages/<编号>/`、没有标记，接手的程序（网页软件）照「认不出是 Miyu 的数据根」拒绝（桌面端 10-31 在 macOS 上用
    // 空的数据根第一次开 `miyu web` 撞到）。认不出、建不成的照原话说，不跑。
    if let Ok(root) = DataRoot::locate(&Env::current())
        && let Err(error) = root.prepare()
    {
        crate::diagnostic::error(
            err,
            language,
            &crate::diagnostic::Problem::new(error.to_string()),
        );
        return Some(1);
    }
    // 它的几处跑之前建好、清掉上一回的暂存，交真实的位置（施工 F-15、FH-3下）；建不成的不跑。
    let root = DataRoot::at(one.home.clone());
    let dirs = match PackageDirs::ready(&root, &one.package, dir.to_path_buf()) {
        Ok(dirs) => dirs,
        Err(error) => {
            say(err, &no_dirs(one, &root, &error, language));
            return Some(1);
        }
    };
    Some(run(command(&launch, &rest, &dirs, &one.resources), err))
}

/// 命令行 `args` 转交给哪一个、交给它的参数；不该转交的没有。清单写的参数在前（施工 F-13）：`miyu shellhook install` 转交成
/// `miyu-tui shellhook install`，`miyu help shellhook` 转交成 `miyu-tui shellhook --help`。
fn target<'a>(args: &[OsString], added: &'a [Added]) -> Option<(&'a Added, Vec<OsString>)> {
    let first = args.get(1)?.to_str()?;
    let (name, typed): (&str, Vec<OsString>) = if first == "help" {
        let name = args.get(2)?.to_str()?;
        (name, vec![OsString::from("--help")])
    } else {
        (first, args[2..].to_vec())
    };
    let one = added.iter().find(|one| one.name == name)?;
    Some((
        one,
        one.args.iter().map(OsString::from).chain(typed).collect(),
    ))
}

/// 跑包的程序的命令：照 `launch` 起（Windows 上照 `#!` 起的是解释器加脚本，施工 X-34），参数 `args`，环境照这个进程的，另把它在
/// 各层的几处 `dirs` 照设计 55 第四节的名字交（包照它读自己的文件、写自己的东西，不自己拼数据根的布局；施工 S-6 上、F-15、
/// FH-3下），`MIYU_RESOURCES` 设成找到它的那份资源目录 `resources`（施工 S-6 中：自己装的包程序在包的 `bin/` 里，照「程序旁边」
/// 找不到资源目录；核心拉起时也设这一个），`MIYU_PROGRAM` 设成主程序的真实位置（施工 G-15）。
fn command(
    launch: &Launch,
    args: &[OsString],
    dirs: &PackageDirs,
    resources: &Path,
) -> std::process::Command {
    let mut command = launch.command();
    command
        .args(args)
        .envs(dirs.env())
        .envs(miyu_store::program::named(
            miyu_store::program::RESOURCES,
            resources,
        ));
    if let Some(program) = Env::current().exe {
        command.env(miyu_store::env::PROGRAM, program);
    }
    command
}

/// 它的几处建不成：转交不成，说是哪个包的、系统的原话（施工 F-15）。
fn no_dirs(one: &Added, root: &DataRoot, error: &std::io::Error, language: Language) -> String {
    let state = root.package_state(&one.package);
    let state = state.display();
    match language {
        Language::Chinese => format!(
            "没能转交 miyu {}：建不了它的目录 {state} 这些（{error}）。",
            one.name
        ),
        Language::English => format!(
            "Could not hand miyu {} over: cannot create its directories such as {state} ({error}).",
            one.name
        ),
    }
}

/// 没装程序：哪一份清单说这个子命令由谁跑。
fn not_installed(one: &Added, language: Language) -> String {
    let manifest = one.manifest.display();
    match language {
        Language::Chinese => format!(
            "没找到 {}：清单 {manifest} 说 miyu {} 由它跑。重装这个软件包再试。",
            one.program, one.name
        ),
        Language::English => format!(
            "{} not found: {manifest} says miyu {} runs it. Reinstall the package and try again.",
            one.program, one.name
        ),
    }
}

fn say(err: &mut dyn Write, text: &str) {
    if writeln!(err, "{text}").is_err() {
        // 标准错误关了：没有别处可说。
    }
}

/// Unix 上换成 `command` 的程序：信号、终端都直接到它，它的退出码就是这个进程的。换不成的说为什么，退出码 1。
#[cfg(unix)]
pub(crate) fn run(mut command: std::process::Command, err: &mut dyn Write) -> u8 {
    use std::os::unix::process::CommandExt;
    let error = command.exec();
    say(
        err,
        &format!("{}: {error}", Path::new(command.get_program()).display()),
    );
    1
}
