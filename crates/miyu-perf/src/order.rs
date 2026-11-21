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
//! 4. 两段的日志各一个文件，放在沙盒目录下；清单写完删掉（一份几十 MB）。

mod listing;
mod script;

use std::io::BufReader;
use std::path::Path;
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

    let symbols = Symbols::parse(&nm(&args.miyu)?);
    let mut listing = Listing::new(&symbols);
    for (log, base) in logs.iter().zip([first_base, second_base]) {
        let file =
            std::fs::File::open(log).map_err(|e| format!("读不了 {}：{e}", log.display()))?;
        listing
            .read(base, BufReader::new(file))
            .map_err(|e| format!("读不了 {}：{e}", log.display()))?;
    }
    let names = listing.names();
    let mut text = names.join("\n");
    text.push('\n');
    std::fs::write(order, text).map_err(|e| format!("清单写不进 {}：{e}", order.display()))?;
    for log in &logs {
        std::fs::remove_file(log).map_err(|e| format!("删不掉 {}：{e}", log.display()))?;
    }
    eprintln!(
        "排函数：程序里 {} 个函数，排进清单 {} 个，合计 {} kB（{}）",
        symbols.count(),
        names.len(),
        listing.bytes() / 1024,
        order.display()
    );
    Ok(())
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
