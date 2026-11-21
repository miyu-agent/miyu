//! 排函数（施工 LW-1，`internal/blueprint/perf.md`「排函数」）：核心的发行程序 `miyu` 照第一次执行的先后排代码段。
//! `cargo xtask perf`、`cargo xtask dist` 共用这里的流程：
//!
//! 1. 清单（cargo 的 target 目录下的 `order/miyu.txt`）先写成空文件，带着 `MIYU_LINK_ORDER` 编 release 的核心：符号名
//!    和排过的程序一样，量尺照它的符号表换函数名；
//! 2. 编 release 的量尺 `miyu-perf`，跑 `miyu-perf --order` 在 QEMU 的用户态模拟器里照剧本运行核心，录出清单；
//! 3. 清单变了，再编一遍核心：构建脚本（`crates/miyu/build.rs`）跟着重跑，只有 `miyu` 这个程序重新链接；
//! 4. 核对排上了（`verify.rs`）：换了链接器、参数没生效时在这里报错，不悄悄不排。
//!
//! 只有 Linux x86_64 做（[`SUPPORTED`]）；别的系统上调用方照原来的办法编。

mod verify;

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::dist::{CORE, output, status, target_dir};

/// 这台机器做不做排函数：只有 Linux x86_64 做，别的系统的程序和原来一样（施工单 LW-1「不做什么」）。
pub(crate) const SUPPORTED: bool = cfg!(all(target_os = "linux", target_arch = "x86_64"));

/// `cargo xtask perf`、`cargo xtask dist` 认的参数：带了的照原来的办法编，不排。这个词不往量尺那边传。
pub(crate) const NO_ORDER: &str = "--no-order";

/// 构建脚本读的环境变量：清单的绝对路径（同 `crates/miyu/build.rs`）。
const VARIABLE: &str = "MIYU_LINK_ORDER";

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
    let list = target.join("order").join("miyu.txt");
    let miyu = release.join("miyu");

    say(&format!("排函数：写空清单 {}", list.display()));
    if let Some(dir) = list.parent() {
        fs::create_dir_all(dir).map_err(|error| format!("建不了 {}：{error}", dir.display()))?;
    }
    fs::write(&list, "").map_err(|error| format!("写不了 {}：{error}", list.display()))?;
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
    status(
        Command::new(release.join("miyu-perf"))
            .current_dir(root)
            .arg("--miyu")
            .arg(&miyu)
            .arg("--resources")
            .arg(root.join("resources"))
            .arg("--work")
            .arg(target.join("perf-order"))
            .arg("--tree")
            .arg(root.join("crates"))
            .arg("--order")
            .arg(&list)
            .arg("--emulator")
            .arg(emulator),
    )
    .map_err(|error| format!("录清单：{error}"))?;
    let text =
        fs::read_to_string(&list).map_err(|error| format!("读不了 {}：{error}", list.display()))?;
    if verify::names(&text).is_empty() {
        return Err(format!(
            "录清单：量尺成功退出了，{} 却是空的",
            list.display()
        ));
    }

    say("排函数：照清单重链核心");
    status(&mut core_build(cargo, root, &list, locked))
        .map_err(|error| format!("照清单重链核心：{error}"))?;

    say(&format!("排函数：核对 {}", miyu.display()));
    let symbols = output(
        Command::new("nm")
            .args(["--defined-only", "--numeric-sort", "-S"])
            .arg(&miyu),
    )
    .map_err(|error| format!("核对：{error}"))?;
    let placed = verify::check(&symbols, &text).map_err(|error| format!("核对：{error}"))?;
    say(&format!("排函数：{placed}"));
    Ok(())
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
