//! 排函数的清单（施工 LW-1，`internal/blueprint/perf.md`「排函数」）：核心的发行程序照「真正执行过的函数、头一回执行的先后」
//! 排代码，清单由这里录。
//!
//! 核心放进 QEMU 的用户态模拟器里带 `-d in_asm` 起（[`Traced`]）：模拟器只在一块代码头一回要执行时翻译它、把这一块每条
//! 指令的地址写进日志，日志就是执行过的代码的全集，先后就是头一回执行的先后。量尺照剧本驱动它（`order/script.rs`），停了
//! 以后照 `nm` 的符号表把日志里的地址换成函数名（`order/listing.rs`），写进 `--order` 给的文件。
//!
//! 先后：
//!
//! 1. 原生先起两遍、各停掉：第一遍头问的六样、开一个关着记忆的会话在三个模型上各说一句；第二遍只问六样。数据根建好、
//!    模型目录的摘要缓存好，后面录的是用户平常的样子（第三次启动才是稳定的）。
//! 2. 第一段（模拟器里）：见 [`script::first`]。
//! 3. 第二段（模拟器里，同一个数据根）：见 [`script::second`]。
//! 4. 带了 `--head` 的（施工 LW-6）：再配着真的终端界面录第三段（核心在模拟器里），日志接在核心清单后面；另带了
//!    `--head-order` 的再录第四段（终端界面在模拟器里），日志是终端界面自己的清单，写进 `--head-order`。见 `order/head.rs`。
//! 5. 每段的日志各一个文件，放在沙盒目录下；清单写完删掉（一份几十 MB）。

#[cfg(unix)]
mod head;
mod listing;
#[cfg(unix)]
mod screen;
mod script;
#[cfg(unix)]
mod terminal;

use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::args::Args;
use crate::fake::Fake;
use crate::measure::tools::calls::Workspace;
use crate::sandbox::{Launch, Sandbox, Traced};
use listing::{Listing, Symbols};

/// 录一遍、写清单：清单写到 `order`，核心在模拟器 `emulator` 里起，工具一步的工作区拷 `tree`。
///
/// # Errors
///
/// 哪一步没成（起不来、哪一句没说成、HTTPS 没走 TLS、`nm` 不在、日志读不了、清单写不进），报是在做哪一步。
pub async fn record(args: &Args, order: &Path, emulator: &Path, tree: &Path) -> Result<(), String> {
    let mut fake = Fake::start(args.reply_bytes).await?;
    let secure = fake.secure().await?;
    let sandbox = Sandbox::new(&args.work, &args.miyu, &args.resources, &fake.base_url)?;
    script::configure(&sandbox, &secure)?;
    let trust = args.work.join("trust.pem");
    script::trust(&trust)?;
    let workspace = Workspace::prepare(&sandbox.work.join("tools"), tree)?;
    let native = Launch {
        trust: Some(&trust),
        ..Launch::default()
    };

    eprintln!("排函数：原生先起两遍");
    let running = sandbox.start_with(native)?;
    let warmed = async {
        let mut rpc = script::head(&sandbox).await?;
        script::three_models(&mut rpc, &mut fake, &sandbox, "perf-order-warm").await
    }
    .await;
    sandbox.stop(running)?;
    warmed.map_err(|e| format!("原生的第一遍：{e}"))?;
    let running = sandbox.start_with(native)?;
    let warmed = script::head(&sandbox).await;
    sandbox.stop(running)?;
    drop(warmed.map_err(|e| format!("原生的第二遍：{e}"))?);

    let logs = [args.work.join("order-1.log"), args.work.join("order-2.log")];
    let [first_launch, second_launch] = logs.each_ref().map(|log| Launch {
        traced: Some(Traced {
            emulator,
            log: log.as_path(),
        }),
        ..native
    });
    eprintln!("排函数：第一段（模拟器里）");
    let running = sandbox.start_with(first_launch)?;
    let first_base = running.base(&args.miyu);
    let first = script::first(&sandbox, &mut fake, &workspace).await;
    sandbox.stop(running)?;
    let first_base = first_base?;
    let session = first.map_err(|e| format!("第一段：{e}"))?;
    eprintln!("排函数：第二段（模拟器里，打开第一段的会话）");
    let running = sandbox.start_with(second_launch)?;
    let second_base = running.base(&args.miyu);
    let second = script::second(&sandbox, &mut fake, &session).await;
    sandbox.stop(running)?;
    let second_base = second_base?;
    second.map_err(|e| format!("第二段：{e}"))?;

    let [first_log, second_log] = logs;
    let mut core = vec![(first_log, first_base), (second_log, second_base)];
    let mut head_log = None;
    if let Some(program) = &args.head {
        let fourth = args.head_order.is_some();
        let (third, fourth) =
            with_head(args, &sandbox, &mut fake, emulator, &trust, program, fourth).await?;
        core.push(third);
        head_log = args.head_order.as_deref().zip(fourth);
    }
    write(&args.miyu, &core, order)?;
    if let (Some(program), Some((head_order, log))) = (&args.head, head_log)
        && write(program, &[log], head_order)? == 0
    {
        return Err(format!(
            "终端界面的清单是空的（{}）：第四段的日志里一个终端界面的函数都没认出来",
            head_order.display()
        ));
    }
    Ok(())
}

/// 配着真的终端界面录第三段、`fourth` 的再录第四段（施工 LW-6，`order/head.rs`）：交回核心、终端界面各自的日志和装载起点。
#[cfg(unix)]
async fn with_head(
    args: &Args,
    sandbox: &Sandbox,
    fake: &mut Fake,
    emulator: &Path,
    trust: &Path,
    program: &Path,
    fourth: bool,
) -> Result<((PathBuf, u64), Option<(PathBuf, u64)>), String> {
    let logs = head::record(args, sandbox, fake, emulator, trust, program, fourth).await?;
    Ok((logs.core, logs.head))
}

/// 伪终端只在 Unix 上开（`order/terminal.rs`）；排函数本来也只在 Linux 上做。
#[cfg(not(unix))]
async fn with_head(
    _: &Args,
    _: &Sandbox,
    _: &mut Fake,
    _: &Path,
    _: &Path,
    _: &Path,
    _: bool,
) -> Result<((PathBuf, u64), Option<(PathBuf, u64)>), String> {
    Err("配着终端界面录（--head）只在 Unix 上做".to_string())
}

/// 照 `program` 的符号表读几段日志（照先后，各带程序在那一段里的装载起点），清单写进 `order`，日志删掉：交回清单里几个函数。
///
/// # Errors
///
/// `nm` 不在、日志读不了、清单写不进、日志删不掉。
fn write(program: &Path, logs: &[(PathBuf, u64)], order: &Path) -> Result<usize, String> {
    let symbols = Symbols::parse(&nm(program)?);
    let mut listing = Listing::new(&symbols);
    for (log, base) in logs {
        let file =
            std::fs::File::open(log).map_err(|e| format!("读不了 {}：{e}", log.display()))?;
        listing
            .read(*base, BufReader::new(file))
            .map_err(|e| format!("读不了 {}：{e}", log.display()))?;
    }
    let names = listing.names();
    let mut text = names.join("\n");
    text.push('\n');
    std::fs::write(order, text).map_err(|e| format!("清单写不进 {}：{e}", order.display()))?;
    for (log, _) in logs {
        std::fs::remove_file(log).map_err(|e| format!("删不掉 {}：{e}", log.display()))?;
    }
    eprintln!(
        "排函数：程序里 {} 个函数，排进清单 {} 个，合计 {} kB（{}）",
        symbols.count(),
        names.len(),
        listing.bytes() / 1024,
        order.display()
    );
    Ok(names.len())
}

/// `nm --defined-only --numeric-sort -S <program>` 的输出。
///
/// # Errors
///
/// `nm` 不在（要装 binutils）、它报了错。
fn nm(program: &Path) -> Result<String, String> {
    let output = Command::new("nm")
        .args(["--defined-only", "--numeric-sort", "-S"])
        .arg(program)
        .output()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => {
                "找不到 nm：排函数要读符号表，装上 binutils".to_string()
            }
            _ => format!("拉不起 nm：{e}"),
        })?;
    if !output.status.success() {
        return Err(format!(
            "nm 读不了 {} 的符号表：{}",
            program.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout).map_err(|e| format!("nm 写来的不是 UTF-8：{e}"))
}
