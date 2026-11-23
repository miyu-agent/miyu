//! macOS 的发行包照起点清单排函数（施工 MO-3，总施工单 `internal/construction/MO-macOS的发行包排函数.md` 第 4、5 条，
//! `internal/blueprint/perf.md`「排函数」的 macOS 一款）：`cargo xtask dist` 在 macOS aarch64 上用它。
//!
//! 1. 编任何东西之前（[`prepare`]）：找 `nm` 和真的 `cc`，读两份起点清单（`xtask/order/macos-miyu.txt`、
//!    `macos-miyu-tui.txt`），在 `<target>/order/shim/<指纹>/cc` 写垫片脚本（`link.rs` 说明它的两层）。
//! 2. 编核心的三个程序、编终端界面（[`Ordered::build`]）：命令照原样，只多一个 `--config
//!    target.aarch64-apple-darwin.linker="<垫片>"` 和两个环境变量（`link::NAMES`、`link::REPORT`）。编完读垫片的报告，印
//!    「清单几个名字、对上几个、写出几个符号、读符号表用了多久」，对上的不到一半时提醒起点清单太旧（不算失败）。
//! 3. 搭好发行包目录以后，`dist.rs` 对目录里的 `miyu`、`miyu-tui` 各用 `order::check` 核对一次（不到九成报错停下），核对用的
//!    是垫片写出的符号清单。
//!
//! 换了链接器以后 cargo 会把全部依赖重编一遍：链接器是 rustc 的参数，算在 cargo 判断要不要重编的依据里。所以同一个 target
//! 目录里的每一条编译都要带同一个 `--config`，不然同一批依赖要编两遍；`dist` 在 macOS 上编核心的只有一条（三个包一起编）。
//! 也因为这样，`dist` 里编完核心以后不能再对 `-p miyu` 跑一次不带垫片的 `cargo build`，不然它会重编、盖掉排过的那一份。
//! 网页在它自己的检出、自己的 target 里只编一次，也不排，不带垫片。
//!
//! 垫片脚本放在以起点清单的指纹命名的目录里：清单改了，链接器的路径跟着变，cargo 自己就知道要重编、重链；清单没变、cargo
//! 什么都没重链时，上一次链接写的报告照样对得上这一份程序。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

use super::link::{CC, NAMES, REPORT, Report};
use crate::dist::status;

/// 发的那一种 macOS 的目标三元组：垫片经 `target.<三元组>.linker` 交给 cargo。本机编本机时它也管构建脚本和过程宏。
const TRIPLE: &str = "aarch64-apple-darwin";

/// 起点清单在仓库里的目录。
const SEEDS: &str = "xtask/order";

/// 对上的名字不到这个比例（分子、分母）时提醒起点清单太旧。
const STALE: (usize, usize) = (1, 2);

/// 一个要排的程序：它的起点清单、垫片、报告。
pub(crate) struct Ordered {
    /// 说给人听的名字：「核心」「终端界面」。
    label: &'static str,
    /// 起点清单的绝对路径。
    seed: PathBuf,
    /// 垫片脚本的绝对路径。
    shim: PathBuf,
    /// 垫片写报告的地方；符号清单写在它旁边（`link.rs`）。
    report: PathBuf,
}

/// 两个要排的程序。
pub(crate) struct Shims {
    /// 核心的 `miyu`。
    pub(crate) core: Ordered,
    /// 终端界面的 `miyu-tui`。
    pub(crate) tui: Ordered,
}

/// 编之前的准备（见模块的说明第 1 条）。`root` 是仓库，`target` 是核心的 cargo target 目录。
///
/// # Errors
///
/// 缺 `nm` 或者真的 `cc`、起点清单读不了或者是空的、垫片写不进。
pub(crate) fn prepare(root: &Path, target: &Path) -> Result<Shims, String> {
    if !Path::new(CC).is_file() {
        return Err(format!(
            "找不到 {CC}：macOS 上排函数要 Xcode 的命令行工具（xcode-select --install）"
        ));
    }
    match Command::new("nm").arg("--version").output() {
        Ok(done) if done.status.success() => {}
        Ok(done) => return Err(format!("nm --version 没成功（{}）", done.status)),
        Err(error) => {
            return Err(format!(
                "nm 起不来（{error}）：macOS 上排函数要 Xcode 的命令行工具（xcode-select --install）"
            ));
        }
    }
    let xtask =
        std::env::current_exe().map_err(|error| format!("找不到 xtask 自己的程序（{error}）"))?;
    let xtask = xtask
        .to_str()
        .ok_or_else(|| format!("xtask 的路径不是 UTF-8：{}", xtask.display()))?;
    let one = |label, program: &str| -> Result<Ordered, String> {
        let seed = root.join(SEEDS).join(format!("macos-{program}.txt"));
        let text = fs::read_to_string(&seed)
            .map_err(|error| format!("起点清单 {} 读不了（{error}）", seed.display()))?;
        if super::verify::names(&text).is_empty() {
            return Err(format!("起点清单 {} 是空的", seed.display()));
        }
        let shim = target
            .join("order")
            .join("shim")
            .join(fingerprint(&text))
            .join("cc");
        write_shim(&shim, xtask)?;
        let report = target
            .join("order")
            .join("macos")
            .join(format!("{program}.json"));
        make_parent(&report)?;
        Ok(Ordered {
            label,
            seed,
            shim,
            report,
        })
    };
    Ok(Shims {
        core: one("核心", "miyu")?,
        tui: one("终端界面", "miyu-tui")?,
    })
}

impl Ordered {
    /// 带着垫片跑编译命令 `command`（`cargo build …`），读垫片的报告、说一行，交回这一次的符号清单（核对时用）。
    ///
    /// # Errors
    ///
    /// 编不过（多半是垫片自己报了错，原因在 cargo 印出的链接错误里）、报告读不了。
    pub(crate) fn build(
        &self,
        command: &mut Command,
        say: &dyn Fn(&str),
    ) -> Result<PathBuf, String> {
        self.linker(command)?
            .env(NAMES, &self.seed)
            .env(REPORT, &self.report);
        status(command).map_err(|error| format!("带着排函数的垫片编{}：{error}", self.label))?;
        let text = fs::read_to_string(&self.report).map_err(|error| {
            format!(
                "读不了{}的垫片报告 {}（{error}）：这一次没有经垫片链接它。删掉这个程序的 release 产物（cargo clean \
                 --release -p <包>）再打一次",
                self.label,
                self.report.display()
            )
        })?;
        let report = Report::parse(&text)
            .map_err(|error| format!("{} 读不懂（{error}）", self.report.display()))?;
        for line in describe(self.label, &report) {
            say(&line);
        }
        Ok(report.order)
    }
}

impl Ordered {
    /// 只给 `command` 加上同一个 `--config`、不设 `link::NAMES`（施工 MO-4）：垫片原样调 `cc`。同一个 target 目录里编别的
    /// 程序（量尺）用它，链接器的参数一样，共同的依赖不用重编。
    ///
    /// # Errors
    ///
    /// 垫片的路径不是 UTF-8、带控制字符。
    pub(crate) fn linker<'c>(&self, command: &'c mut Command) -> Result<&'c mut Command, String> {
        Ok(command.arg("--config").arg(config(&self.shim)?))
    }

    /// 起点清单的绝对路径。
    pub(crate) fn seed(&self) -> &Path {
        &self.seed
    }

    /// 说给人听的名字。
    pub(crate) fn label(&self) -> &'static str {
        self.label
    }
}

/// 报告说成一两行：第一行是几个数，对上的不到一半时第二行提醒。
fn describe(label: &str, report: &Report) -> Vec<String> {
    let percent = (report.found * 100).checked_div(report.listed).unwrap_or(0);
    let mut lines = vec![format!(
        "排函数：{label}的起点清单 {} 个名字，对上 {} 个（{percent}%），写出 {} 个符号，读符号表用了 {}.{:02} 秒",
        report.listed,
        report.found,
        report.symbols,
        report.millis / 1000,
        report.millis % 1000 / 10
    )];
    if report.found * STALE.1 < report.listed * STALE.0 {
        lines.push(format!(
            "排函数：提醒：{label}的起点清单太旧，只对上不到一半的名字，这一次排的效果打了折扣（清单在 {SEEDS}/）"
        ));
    }
    lines
}

/// 交给 cargo 的 `--config`：`target.<三元组>.linker="<垫片>"`，路径写成 TOML 的基本字符串。
///
/// # Errors
///
/// 路径不是 UTF-8、带控制字符。
fn config(shim: &Path) -> Result<String, String> {
    let path = shim
        .to_str()
        .ok_or_else(|| format!("垫片的路径不是 UTF-8：{}", shim.display()))?;
    if path.chars().any(char::is_control) {
        return Err(format!("垫片的路径里有控制字符：{path:?}"));
    }
    let escaped = path.replace('\\', "\\\\").replace('"', "\\\"");
    Ok(format!("target.{TRIPLE}.linker=\"{escaped}\""))
}

/// 垫片脚本的文本：`link::NAMES` 没设的原样调真的 `cc`，设了的交给 `xtask order-link`。
fn shim_script(xtask: &str) -> String {
    let quoted = xtask.replace('\'', "'\\''");
    format!(
        "#!/bin/sh\n\
         # 排函数的链接器垫片：cargo xtask dist 在 macOS 上写的（施工 MO-3，xtask/src/order/link.rs）。\n\
         if [ -z \"${{{NAMES}:-}}\" ]; then\n  exec {CC} \"$@\"\nfi\n\
         exec '{quoted}' order-link \"$@\"\n"
    )
}

/// 起点清单文本的指纹：SHA-256 的前 16 位十六进制。
fn fingerprint(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// 写垫片脚本、设成可执行。
fn write_shim(shim: &Path, xtask: &str) -> Result<(), String> {
    make_parent(shim)?;
    fs::write(shim, shim_script(xtask))
        .map_err(|error| format!("写不进垫片 {}（{error}）", shim.display()))?;
    executable(shim)
}

#[cfg(unix)]
fn executable(file: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(file, fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("设不成可执行 {}（{error}）", file.display()))
}

/// 垫片是 shell 脚本，只在 macOS 上用（`order::WAY`）。
#[cfg(not(unix))]
fn executable(file: &Path) -> Result<(), String> {
    Err(format!(
        "{} 是 shell 脚本，这个系统上用不了",
        file.display()
    ))
}

fn make_parent(file: &Path) -> Result<(), String> {
    match file.parent() {
        Some(dir) => {
            fs::create_dir_all(dir).map_err(|error| format!("建不了 {}（{error}）", dir.display()))
        }
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests;
