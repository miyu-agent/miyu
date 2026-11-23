//! 排函数（施工 LW-1，`internal/blueprint/perf.md`「排函数」）：核心的发行程序 `miyu` 照第一次执行的先后排代码段。
//! `cargo xtask perf`、`cargo xtask dist` 共用这里的流程：
//!
//! 1. 清单（cargo 的 target 目录下的 `order/miyu.txt`）先写成空文件，带着 `MIYU_LINK_ORDER` 编 release 的核心：符号名
//!    和排过的程序一样，量尺照它的符号表换函数名；
//! 2. 编 release 的量尺 `miyu-perf`，跑 `miyu-perf --order` 在 QEMU 的用户态模拟器里照剧本运行核心，录出清单；
//! 3. 清单变了，再编一遍核心：构建脚本（`crates/miyu/build.rs`）跟着重跑，只有 `miyu` 这个程序重新链接；
//! 4. 核对排上了（`verify.rs`）：换了链接器、参数没生效时在这里报错，不悄悄不排。
//!
//! `cargo xtask dist` 另在搭好发行包目录以后对着它再录一遍（施工 LW-6，[`staged`]）：量尺配着真的终端界面多录两段，核心
//! 照新清单重链，终端界面认清单的（[`tui_list`]）带着它自己的清单再编一遍；`dist.rs` 把两个程序重新拷进目录、核对（[`check`]）。
//!
//! 上面是 Linux x86_64 的做法。macOS aarch64 上不录、不重链，照仓库里的起点清单经链接器垫片排（施工 MO-3，`macos.rs`、
//! `link.rs`、`seed.rs`），只在 `cargo xtask dist` 里做。别的系统不排，调用方照原来的办法编（[`WAY`]）。

pub(crate) mod adopt;
pub(crate) mod link;
pub(crate) mod macos;
pub(crate) mod probe;
mod seed;
mod verify;

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::dist::{CORE, head_cargo, output, status, target_dir};

/// 一台机器怎么排函数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Way {
    /// 在 QEMU 的用户态模拟器里录清单、重链（施工 LW-1、LW-6）：Linux x86_64。
    Emulator,
    /// 照仓库里的起点清单经链接器垫片排（施工 MO-3，`macos.rs`）：macOS aarch64，只在 `cargo xtask dist` 里做。
    Shim,
    /// 不排，程序和原来一样。
    Off,
}

/// 这台机器怎么排函数：Linux x86_64 用模拟器，macOS aarch64 用垫片，别的系统不排（施工单 LW-1「不做什么」、总施工单 MO
/// 第 6 条：Intel 的 Mac 不做）。
pub(crate) const WAY: Way = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
    Way::Emulator
} else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
    Way::Shim
} else {
    Way::Off
};

/// `cargo xtask perf`、`cargo xtask dist` 认的参数：带了的照原来的办法编，不排。这个词不往量尺那边传。
pub(crate) const NO_ORDER: &str = "--no-order";

/// `cargo xtask dist` 认的参数（施工 MO-4）：macOS 上照起点清单排、照样核对，只是不带探针跑那一轮、不写新的起点清单。
/// 别的系统上不起作用。
pub(crate) const NO_PROBE: &str = "--no-probe";

/// 构建脚本读的环境变量：清单的绝对路径（同 `crates/miyu/build.rs`，终端界面仓库的 `build.rs` 认同一个）。
pub(crate) const VARIABLE: &str = "MIYU_LINK_ORDER";

/// 终端界面的程序名（施工 LW-6）：在发行包目录里的名字，它的清单也照它起名。
const TUI: &str = "miyu-tui";

/// QEMU 用户态模拟器的程序名。
const EMULATOR: &str = "qemu-x86_64";

/// 排函数要用的外面的程序：交回模拟器在 `PATH` 里的位置；`nm` 先试过一次，免得编了半天到核对才发现没有。调用方在编任何
/// 东西之前先调它，缺了什么当场报。
///
/// # Errors
///
/// 找不到模拟器、跑不了 `nm` 的，交回要装什么。
pub(crate) fn tools() -> Result<PathBuf, String> {
    let emulator = std::env::var_os("PATH")
        .iter()
        .flat_map(std::env::split_paths)
        .map(|dir| dir.join(EMULATOR))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| {
            format!(
                "PATH 里找不到 {EMULATOR}：排函数要 QEMU 的用户态模拟器（Debian、Ubuntu 的包是 qemu-user，Arch 的是 \
                 qemu-user），不排可以加 {NO_ORDER}"
            )
        })?;
    match Command::new("nm").arg("--version").output() {
        Ok(done) if done.status.success() => Ok(emulator),
        Ok(done) => Err(format!("nm --version 没成功（{}）", done.status)),
        Err(error) if error.kind() == ErrorKind::NotFound => Err(
            "找不到 nm：核对排函数要 binutils 里的 nm（Debian、Ubuntu、Arch 的包都叫 binutils）"
                .to_string(),
        ),
        Err(error) => Err(format!("nm 起不来（{error}）")),
    }
}

/// 整个流程：写空清单、编核心、编量尺、录清单、重链核心、核对。`locked` 是给 cargo 的 `--locked`（`dist` 带，`perf` 不带）；
/// `say` 照调用方各自的说法在标准输出上说一行在做什么。
///
/// # Errors
///
/// 哪一步不成就停在哪一步，交回那一步和那条命令的报错：编不过、量尺没成功、录出来的清单是空的、核对没排上。
pub(crate) fn run(
    cargo: &str,
    root: &Path,
    emulator: &Path,
    locked: bool,
    say: &dyn Fn(&str),
) -> Result<(), String> {
    let target = target_dir(root, cargo)?;
    let release = target.join("release");
    let list = core_list(&target);
    let miyu = release.join("miyu");

    say(&format!("排函数：写空清单 {}", list.display()));
    blank(&list)?;
    say("排函数：带空清单编核心");
    status(&mut core_build(cargo, root, &list, locked))
        .map_err(|error| format!("带空清单编核心：{error}"))?;

    say("排函数：编量尺");
    let mut perf = Command::new(cargo);
    perf.current_dir(root).args(["build", "--release"]);
    if locked {
        perf.arg("--locked");
    }
    status(perf.args(["-p", "miyu-perf"])).map_err(|error| format!("编量尺：{error}"))?;

    say(&format!("排函数：在 {} 里录清单", emulator.display()));
    let paths = Recording {
        miyu: &miyu,
        resources: &root.join("resources"),
        work: &target.join("perf-order"),
        tree: &root.join("crates"),
        list: &list,
        flag: "--emulator",
        recorder: emulator,
    };
    let mut command = recording(&release.join("miyu-perf"), &paths);
    status(command.current_dir(root)).map_err(|error| format!("录清单：{error}"))?;
    listed(&list).map_err(|error| format!("录清单：{error}"))?;

    say("排函数：照清单重链核心");
    status(&mut core_build(cargo, root, &list, locked))
        .map_err(|error| format!("照清单重链核心：{error}"))?;

    say(&format!("排函数：核对 {}", miyu.display()));
    let placed = check(&miyu, &list).map_err(|error| format!("核对：{error}"))?;
    say(&format!("排函数：{placed}"));
    Ok(())
}

/// 核心的清单：cargo 的 target 目录下的 `order/miyu.txt`。
pub(crate) fn core_list(target: &Path) -> PathBuf {
    target.join("order").join("miyu.txt")
}

/// 终端界面的清单（施工 LW-6）：检出 `checkout` 里有认 `MIYU_LINK_ORDER` 的构建脚本的，写成空文件交回
/// `<target>/order/miyu-tui.txt`，编终端界面时设上它；不认的在标准输出上说一句，交回 `None`，终端界面照原来的办法编。
///
/// # Errors
///
/// 构建脚本在却读不了、清单写不进。
pub(crate) fn tui_list(checkout: &Path, target: &Path) -> Result<Option<PathBuf>, String> {
    if !takes_order(checkout)? {
        println!("终端界面钉的提交还不认排函数，这一次不排它");
        return Ok(None);
    }
    let list = target.join("order").join(format!("{TUI}.txt"));
    blank(&list)?;
    Ok(Some(list))
}

/// 检出 `checkout` 认不认排函数的清单：有 `build.rs`、里面写着 [`VARIABLE`]（终端界面仓库施工 LW-4 加的）。
///
/// # Errors
///
/// `build.rs` 在却读不了。
fn takes_order(checkout: &Path) -> Result<bool, String> {
    let script = checkout.join("build.rs");
    match fs::read_to_string(&script) {
        Ok(text) => Ok(text.contains(VARIABLE)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("读不了 {}：{error}", script.display())),
    }
}

/// 发行包目录搭好以后再录的一遍（施工 LW-6）用到的几处。
pub(crate) struct Staged<'a> {
    /// 搭好的发行包目录：量尺照它的 `miyu`、`resources/`、`miyu-tui` 录。
    pub(crate) dir: &'a Path,
    /// 终端界面的检出：带着它的清单再编一遍。
    pub(crate) tui: &'a Path,
    /// 终端界面的清单（[`tui_list`]）；终端界面不认清单的是 `None`，这时只录核心的第三段。
    pub(crate) tui_list: Option<&'a Path>,
}

/// 对着搭好的发行包目录再录一遍、照新清单重编（施工 LW-6，`internal/blueprint/perf.md`「排函数」）：量尺带上 `--head`
/// （和 `--head-order`）整份重写核心清单（前两段也重录，清单以对着真发行包录的为准）、写终端界面的清单；核心重链，终端界面
/// 带着清单再编。拷回发行包目录、核对由调用方做（`dist.rs`）。`say` 同 [`run`]。
///
/// # Errors
///
/// 哪一步不成就停在哪一步：量尺没成功、清单是空的、编不过。
pub(crate) fn staged(
    cargo: &str,
    root: &Path,
    emulator: &Path,
    staged: &Staged<'_>,
    say: &dyn Fn(&str),
) -> Result<(), String> {
    let target = target_dir(root, cargo)?;
    let list = core_list(&target);
    say(&format!(
        "排函数：对着 {} 配着真的终端界面再录一遍",
        staged.dir.display()
    ));
    let paths = Recording {
        miyu: &staged.dir.join("miyu"),
        resources: &staged.dir.join("resources"),
        work: &target.join("perf-order"),
        tree: &root.join("crates"),
        list: &list,
        flag: "--emulator",
        recorder: emulator,
    };
    let mut command = recording(&target.join("release").join("miyu-perf"), &paths);
    command.arg("--head").arg(staged.dir.join(TUI));
    if let Some(tui_list) = staged.tui_list {
        command.arg("--head-order").arg(tui_list);
    }
    status(command.current_dir(root)).map_err(|error| format!("对着发行包录清单：{error}"))?;
    listed(&list).map_err(|error| format!("对着发行包录清单：{error}"))?;
    say("排函数：照新清单重链核心");
    status(&mut core_build(cargo, root, &list, true))
        .map_err(|error| format!("照新清单重链核心：{error}"))?;
    if let Some(tui_list) = staged.tui_list {
        listed(tui_list).map_err(|error| format!("对着发行包录清单：{error}"))?;
        say("排函数：带着终端界面的清单再编终端界面");
        status(head_cargo(staged.tui).env(VARIABLE, tui_list).args([
            "build",
            "--release",
            "--locked",
        ]))
        .map_err(|error| format!("带着清单编终端界面：{error}"))?;
    }
    Ok(())
}

/// 核对程序 `program`（核心的 `miyu`、终端界面都行）照清单 `list` 排上了：`nm` 读符号表，交给 [`verify::check`]。
///
/// # Errors
///
/// `nm` 不成、清单读不了、没排上。
pub(crate) fn check(program: &Path, list: &Path) -> Result<verify::Placed, String> {
    let text =
        fs::read_to_string(list).map_err(|error| format!("读不了 {}：{error}", list.display()))?;
    let symbols = output(
        Command::new("nm")
            .args(["--defined-only", "--numeric-sort", "-S"])
            .arg(program),
    )?;
    verify::check(&symbols, &text)
}

/// 把清单写成空文件（上一级没有的先建）。
fn blank(list: &Path) -> Result<(), String> {
    if let Some(dir) = list.parent() {
        fs::create_dir_all(dir).map_err(|error| format!("建不了 {}：{error}", dir.display()))?;
    }
    fs::write(list, "").map_err(|error| format!("写不了 {}：{error}", list.display()))
}

/// 量尺录好的清单不能是空的：量尺成功退出了、清单却空着，说明哪里没录到，不拿它去链接。
fn listed(list: &Path) -> Result<(), String> {
    let text =
        fs::read_to_string(list).map_err(|error| format!("读不了 {}：{error}", list.display()))?;
    match verify::names(&text).is_empty() {
        true => Err(format!("量尺成功退出了，{} 却是空的", list.display())),
        false => Ok(()),
    }
}

/// 录清单的量尺要的几处路径。
struct Recording<'a> {
    miyu: &'a Path,
    resources: &'a Path,
    work: &'a Path,
    tree: &'a Path,
    list: &'a Path,
    /// `--emulator`（Linux）或者 `--probe`（macOS，施工 MO-4）。
    flag: &'a str,
    /// 模拟器或者探针的动态库。
    recorder: &'a Path,
}

/// `miyu-perf --order` 的命令行（`perf` 是量尺的程序）。
fn recording(perf: &Path, paths: &Recording<'_>) -> Command {
    let mut command = Command::new(perf);
    command
        .arg("--miyu")
        .arg(paths.miyu)
        .arg("--resources")
        .arg(paths.resources)
        .arg("--work")
        .arg(paths.work)
        .arg("--tree")
        .arg(paths.tree)
        .arg("--order")
        .arg(paths.list)
        .arg(paths.flag)
        .arg(paths.recorder);
    command
}

/// 编核心的三个程序，带着清单（只有 `miyu` 的链接用它，见 `crates/miyu/build.rs`）。
fn core_build(cargo: &str, root: &Path, list: &Path, locked: bool) -> Command {
    let mut command = Command::new(cargo);
    command
        .current_dir(root)
        .env(VARIABLE, list)
        .args(["build", "--release"]);
    if locked {
        command.arg("--locked");
    }
    command.args(CORE.iter().flat_map(|name| ["-p", name]));
    command
}

#[cfg(test)]
mod tests;
