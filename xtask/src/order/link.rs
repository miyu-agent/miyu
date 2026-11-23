//! `cargo xtask order-link <cc 的参数…>`：macOS 排函数的链接器垫片的第二层（施工 MO-3，总施工单
//! `internal/construction/MO-macOS的发行包排函数.md` 第 4 条）。
//!
//! 第一层是 `macos.rs` 写在 `<target>/order/shim/<指纹>/cc` 的 shell 脚本，cargo 经 `--config
//! target.aarch64-apple-darwin.linker` 把它当链接器交给 rustc。环境变量 [`NAMES`] 没设的，脚本直接调真的 [`CC`]；设了的，
//! 调这个子命令。它做的事：
//!
//! 1. 认 `-o` 后面的输出：只有 `deps/miyu-<16 位十六进制>`（核心）、`deps/miyu_tui-<16 位十六进制>`（终端界面）才排，别的
//!    （`miyu-sandbox`、`miyu-extract`、`miyu-watchdog`、构建脚本、过程宏）原样交给 [`CC`]。核心的几个包一起编、构建脚本也经过垫片，是因为拆开编
//!    时 cargo 给共同依赖合的特性不一样，编出来的核心就不是同一份（前置任务实测 `getrandom` 的特性不同）。
//! 2. 要排的，从参数里找输入：输出所在的 `deps/` 下面的 `.o`（LTO 以后的目标文件）和 `.rlib`、`.a`（rustc 链接时在 `deps/`
//!    下面建的临时目录里，带着 zstd、sqlite、ring 的 C 部分）；工具链自带的库、`-l`、`-framework` 不读。`@` 开头的参数是
//!    响应文件，先读开再认（现在的链接没有用，rustc 换版本以后可能用）。
//! 3. 对这些输入跑一次 `nm --defined-only`，把起点清单（[`NAMES`] 指的文件）换成这一次的符号（`seed.rs`），写进报告
//!    （[`REPORT`]）旁边的符号清单，报告写下几个数。
//! 4. 调真的 [`CC`]，参数是原来的参数加 `-Wl,-order_file,<符号清单>`，退出码照它的。链接器的警告（例如 zstd-sys 的目标
//!    文件比部署目标新的 42 行 `ld: warning`）照原样印，不算失败。
//!
//! 垫片自己出错（报告的变量没设、起点清单读不了、`nm` 跑不了、写不进）时，在标准错误上说清楚是哪一样，退出码非零，不调
//! [`CC`]：rustc 报链接失败，cargo 停下，`dist` 跟着停下，不悄悄地不排。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

use super::seed;

/// 起点清单的绝对路径：垫片脚本照它有没有设决定排不排。
pub(crate) const NAMES: &str = "MIYU_ORDER_NAMES";

/// 报告文件的绝对路径（JSON）。符号清单写在它旁边，扩展名换成 [`ORDER_EXTENSION`]。
pub(crate) const REPORT: &str = "MIYU_ORDER_REPORT";

/// 符号清单的扩展名。
const ORDER_EXTENSION: &str = "order";

/// 真的链接器驱动。写绝对路径：垫片脚本自己也叫 `cc`（rustc 照链接器的文件名认它是哪一种），照 `PATH` 找可能找回自己。
pub(crate) const CC: &str = "/usr/bin/cc";

/// 要排的两个程序在 cargo 的 `deps/` 里的名字前缀：后面跟 `-` 和 16 位十六进制的哈希。
const ORDERED: [&str; 2] = ["miyu", "miyu_tui"];

/// 垫片写的报告：xtask 编完以后读出来印（`macos.rs`）。不靠链接器的标准错误：rustc 在链接成功时不一定把它印出来。
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Report {
    /// 起点清单里不重复的名字数。
    pub(crate) listed: usize,
    /// 其中对上这一次的符号的。
    pub(crate) found: usize,
    /// 写进符号清单的符号数。
    pub(crate) symbols: usize,
    /// 跑 `nm`、换名字一共用了多少毫秒。
    pub(crate) millis: u64,
    /// 符号清单的路径：核对排上了没有时用它（`verify.rs`）。
    pub(crate) order: PathBuf,
}

impl Report {
    /// 写成 JSON。
    fn to_json(&self) -> String {
        serde_json::json!({
            "listed": self.listed,
            "found": self.found,
            "symbols": self.symbols,
            "millis": self.millis,
            "order": self.order.to_string_lossy(),
        })
        .to_string()
    }

    /// 读 [`Report::to_json`] 写的。
    ///
    /// # Errors
    ///
    /// 不是 JSON、少了哪一项。
    pub(crate) fn parse(text: &str) -> Result<Report, String> {
        let json: serde_json::Value =
            serde_json::from_str(text).map_err(|error| format!("不是 JSON（{error}）"))?;
        let count = |key: &str| {
            let value = json[key].as_u64().ok_or_else(|| format!("少了 {key}"))?;
            usize::try_from(value).map_err(|_| format!("{key} 太大：{value}"))
        };
        Ok(Report {
            listed: count("listed")?,
            found: count("found")?,
            symbols: count("symbols")?,
            millis: json["millis"].as_u64().ok_or("少了 millis")?,
            order: json["order"]
                .as_str()
                .map(PathBuf::from)
                .ok_or("少了 order")?,
        })
    }
}

/// `cargo xtask order-link <cc 的参数…>`。
pub fn run(args: &[String]) -> ExitCode {
    match link(args) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("order-link（排函数的链接器垫片）：{error}");
            ExitCode::from(2)
        }
    }
}

fn link(args: &[String]) -> Result<ExitCode, String> {
    let expanded = expand(args)?;
    let extra = match output(&expanded).filter(|out| wanted(out)) {
        Some(out) => vec![order(&expanded, out)?],
        None => Vec::new(),
    };
    let done = Command::new(CC)
        .args(args)
        .args(&extra)
        .status()
        .map_err(|error| format!("{CC} 起不来（{error}）"))?;
    // 被信号停下的没有退出码：照失败报给 rustc。
    let code = done
        .code()
        .map_or(1, |code| u8::try_from(code).unwrap_or(1));
    Ok(ExitCode::from(code))
}

/// 照起点清单写出这一次的符号清单和报告，交回要加给 `cc` 的参数 `-Wl,-order_file,<符号清单>`。
fn order(args: &[String], out: &Path) -> Result<String, String> {
    let report = std::env::var_os(REPORT)
        .map(PathBuf::from)
        .ok_or_else(|| format!("设了 {NAMES} 却没设 {REPORT}"))?;
    let names = std::env::var_os(NAMES)
        .map(PathBuf::from)
        .ok_or_else(|| format!("没设 {NAMES}"))?;
    let names = fs::read_to_string(&names)
        .map_err(|error| format!("起点清单 {} 读不了（{error}）", names.display()))?;
    let order = report.with_extension(ORDER_EXTENSION);
    let flag = order
        .to_str()
        .ok_or_else(|| format!("符号清单的路径不是 UTF-8：{}", order.display()))?;
    // `-Wl,` 后面照逗号拆成几个参数：路径里有逗号就拆错了。
    if flag.contains(',') {
        return Err(format!("符号清单的路径里有逗号，交不给链接器：{flag}"));
    }
    let flag = format!("-Wl,-order_file,{flag}");
    let started = Instant::now();
    let files = inputs(args, out);
    if files.is_empty() {
        return Err(format!(
            "参数里找不到 {} 下面的 .o、.rlib、.a：rustc 给链接器的参数和以前不一样了",
            out.parent().unwrap_or(out).display()
        ));
    }
    let listing = Command::new("nm")
        .arg("--defined-only")
        .args(&files)
        .output()
        .map_err(|error| format!("nm 起不来（{error}）：它是 Xcode 命令行工具自带的"))?;
    if !listing.status.success() {
        return Err(format!(
            "nm 读 {} 个输入没成功（{}）：{}",
            files.len(),
            listing.status,
            String::from_utf8_lossy(&listing.stderr).trim()
        ));
    }
    let listing = String::from_utf8(listing.stdout).map_err(|_| "nm 的输出不是 UTF-8")?;
    let symbols = seed::code_symbols(&listing);
    let mapped = seed::map(&names, &symbols);
    let millis = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    write(&order, &seed::render(&mapped.symbols))?;
    let written = Report {
        listed: mapped.listed,
        found: mapped.found,
        symbols: mapped.symbols.len(),
        millis,
        order: order.clone(),
    };
    write(&report, &written.to_json())?;
    Ok(flag)
}

fn write(file: &Path, text: &str) -> Result<(), String> {
    fs::write(file, text).map_err(|error| format!("写不进 {}（{error}）", file.display()))
}

/// 展开参数里的响应文件（`@<文件>`）。rustc 给 `cc` 写的响应文件一行一个参数，参数里的反斜杠和空格前面各加一个反斜杠。
///
/// # Errors
///
/// 响应文件读不了。
fn expand(args: &[String]) -> Result<Vec<String>, String> {
    let mut expanded = Vec::new();
    for arg in args {
        match arg.strip_prefix('@') {
            Some(file) => {
                let text = fs::read_to_string(file)
                    .map_err(|error| format!("响应文件 {file} 读不了（{error}）"))?;
                expanded.extend(response(&text));
            }
            None => expanded.push(arg.clone()),
        }
    }
    Ok(expanded)
}

/// 读响应文件的文本（见 [`expand`]）。
fn response(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let mut arg = String::new();
            let mut chars = line.chars();
            while let Some(char) = chars.next() {
                match char {
                    '\\' => arg.extend(chars.next()),
                    other => arg.push(other),
                }
            }
            arg
        })
        .collect()
}

/// `-o` 后面的输出。
fn output(args: &[String]) -> Option<&Path> {
    args.iter()
        .position(|arg| arg == "-o")
        .and_then(|index| args.get(index + 1))
        .map(Path::new)
}

/// 输出是不是要排的两个程序之一：在 `deps/` 下面，名字是 [`ORDERED`] 之一加 `-` 和 16 位十六进制。
fn wanted(out: &Path) -> bool {
    let in_deps = out
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|dir| dir == "deps");
    let named = out
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.rsplit_once('-'))
        .is_some_and(|(stem, hash)| {
            ORDERED.contains(&stem)
                && hash.len() == 16
                && hash.bytes().all(|b| b.is_ascii_hexdigit())
        });
    in_deps && named
}

/// 要交给 `nm` 的输入：输出所在目录下面（含 rustc 在那里建的临时目录）的 `.o`、`.rlib`、`.a`。选项（`-` 开头的）不算。
fn inputs<'a>(args: &'a [String], out: &Path) -> Vec<&'a str> {
    let Some(deps) = out.parent() else {
        return Vec::new();
    };
    args.iter()
        .map(String::as_str)
        .filter(|arg| !arg.starts_with('-'))
        .filter(|arg| {
            [".o", ".rlib", ".a"]
                .iter()
                .any(|suffix| arg.ends_with(suffix))
        })
        .filter(|arg| Path::new(arg).starts_with(deps))
        .collect()
}

#[cfg(test)]
mod tests;
