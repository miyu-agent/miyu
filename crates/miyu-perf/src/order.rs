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
//! 5. 抽记忆的那一段（模拟器里，同一个数据根，施工 LW-14）：见 [`script::remembered`]。这一段的沙箱配置另加
//!    [`remember::SETTINGS`]（开着记忆的会话每说 11 轮就抽一次记忆），前面几段的会话照出厂的门槛，不在半路抽。日志排在核心
//!    清单的最后：前面几段的清单和没有这一段时一样，只有抽记忆才头一回执行的函数接在后面。
//! 6. 每段的日志各一个文件，放在沙盒目录下；清单写完删掉（一份几十 MB）。
//!
//! macOS 上（施工 MO-2）没有 QEMU 的用户态模拟器，`--probe` 代替 `--emulator`（[`Recorder`]）：五段的剧本、先后都一样，只是
//! 被测的程序不放进模拟器，而是带着页探针原生起（[`Probed`]）。探针每个进程写一份记录（`order/pages.rs`），写下的已经是
//! 符号表里的地址，不用读装载起点（macOS 也没有 `/proc`）。它一遍只记得到每一页上（每一次重关以后）第一个碰它的函数，
//! 不像模拟器那样记全，所以这一路写出的清单是「这一遍看到的函数」，照第一次看到的先后，格式和模拟器那一路一样。

#[cfg(unix)]
mod head;
mod listing;
mod pages;
mod remember;
#[cfg(unix)]
mod screen;
mod script;
#[cfg(unix)]
mod terminal;
#[cfg(unix)]
mod tour;

use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::args::Args;
use crate::fake::Fake;
use crate::measure::tools::calls::Workspace;
use crate::sandbox::{Launch, Probed, Running, Sandbox, Traced};
use listing::{Listing, Symbols};

/// 用什么录（施工 MO-2）：`--emulator` 或者 `--probe`，二者择一（`args.rs` 查过）。
#[derive(Debug, Clone, Copy)]
pub enum Recorder<'a> {
    /// Linux：放进 QEMU 的用户态模拟器（[`Traced`]），每段一份日志，地址要减去装载起点。
    Emulator(&'a Path),
    /// macOS：插页探针（[`Probed`]），每段一个输出前缀，每个进程一份记录，地址已经是符号表里的。
    Probe(&'a Path),
}

/// 一段录下来的、写清单时读的东西。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trace {
    /// 模拟器的日志，和程序在这一段里的装载起点。
    Log(PathBuf, u64),
    /// 探针的输出前缀：这一段的记录是 `<前缀>.<进程号>`（`order/pages.rs`）。
    Pages(PathBuf),
}

impl<'a> Recorder<'a> {
    /// 第 `name` 段录到哪：模拟器的是 `<work>/<name>.log`，探针的前缀是 `<work>/<name>.pages`。
    fn at(self, work: &Path, name: &str) -> PathBuf {
        match self {
            Recorder::Emulator(_) => work.join(format!("{name}.log")),
            Recorder::Probe(_) => work.join(format!("{name}.pages")),
        }
    }

    /// 照 `plain` 起、录到 `at`、被测程序是 `program` 的起法：放进模拟器，或者带探针。
    fn launch<'b>(self, plain: Launch<'b>, at: &'b Path, program: &'b Path) -> Launch<'b>
    where
        'a: 'b,
    {
        match self {
            Recorder::Emulator(emulator) => Launch {
                traced: Some(Traced { emulator, log: at }),
                ..plain
            },
            Recorder::Probe(library) => Launch {
                probed: Some(Probed {
                    library,
                    out: at,
                    program,
                }),
                ..plain
            },
        }
    }

    /// 刚起来的 `running` 这一段录到 `at`：模拟器的另读程序 `program` 的装载起点（读 `/proc`，[`Running::base`]），探针的
    /// 不读。起来以后马上调，报错留到停了以后再交（和以前一样）。
    fn trace(self, running: &Running, at: &Path, program: &Path) -> Result<Trace, String> {
        match self {
            Recorder::Emulator(_) => Ok(Trace::Log(at.to_path_buf(), running.base(program)?)),
            Recorder::Probe(_) => Ok(Trace::Pages(at.to_path_buf())),
        }
    }

    /// 进度里说核心在哪：「模拟器里」「带探针」。
    fn inside(self) -> &'static str {
        match self {
            Recorder::Emulator(_) => "模拟器里",
            Recorder::Probe(_) => "带探针",
        }
    }
}

/// 录一遍、写清单：清单写到 `order`，核心照 `recorder` 放进模拟器或者带探针起，工具一步的工作区拷 `tree`。
///
/// # Errors
///
/// 哪一步没成（起不来、哪一句没说成、HTTPS 没走 TLS、`nm` 不在、日志读不了、探针没有记录、清单写不进），报是在做哪一步。
pub async fn record(
    args: &Args,
    order: &Path,
    recorder: Recorder<'_>,
    tree: &Path,
) -> Result<(), String> {
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

    let at = ["order-1", "order-2", "order-remembered"].map(|name| recorder.at(&args.work, name));
    let [first_launch, second_launch, remembered_launch] = at
        .each_ref()
        .map(|at| recorder.launch(native, at, &args.miyu));
    let [first_at, second_at, remembered_at] = &at;
    let inside = recorder.inside();
    eprintln!("排函数：第一段（{inside}）");
    let running = sandbox.start_with(first_launch)?;
    let first_trace = recorder.trace(&running, first_at, &args.miyu);
    let first = script::first(&sandbox, &mut fake, &workspace).await;
    sandbox.stop(running)?;
    let first_trace = first_trace?;
    let session = first.map_err(|e| format!("第一段：{e}"))?;
    eprintln!("排函数：第二段（{inside}，打开第一段的会话）");
    let running = sandbox.start_with(second_launch)?;
    let second_trace = recorder.trace(&running, second_at, &args.miyu);
    let second = script::second(&sandbox, &mut fake, &session).await;
    sandbox.stop(running)?;
    let second_trace = second_trace?;
    second.map_err(|e| format!("第二段：{e}"))?;

    let mut core = vec![first_trace, second_trace];
    let mut head_trace = None;
    if let Some(program) = &args.head {
        let fourth = args.head_order.is_some();
        let (third, fourth) =
            with_head(args, &sandbox, &mut fake, recorder, &trust, program, fourth).await?;
        core.push(third);
        head_trace = args.head_order.as_deref().zip(fourth);
    }
    let record = match recorder {
        Recorder::Emulator(_) => "日志",
        Recorder::Probe(_) => "记录",
    };
    eprintln!("排函数：抽记忆的那一段（{inside}，{record}排在核心清单的最后）");
    let config = sandbox.root.config_file();
    let plain = std::fs::read_to_string(&config).map_err(|e| format!("系统配置读不了：{e}"))?;
    std::fs::write(&config, format!("{plain}{}", remember::SETTINGS))
        .map_err(|e| format!("系统配置写不进：{e}"))?;
    let running = sandbox.start_with(remembered_launch)?;
    let remembered_trace = recorder.trace(&running, remembered_at, &args.miyu);
    let remembered = script::remembered(&sandbox, &mut fake).await;
    sandbox.stop(running)?;
    std::fs::write(&config, plain).map_err(|e| format!("系统配置写不进：{e}"))?;
    remembered.map_err(|e| format!("抽记忆的那一段：{e}"))?;
    core.push(remembered_trace?);
    write(&args.miyu, &core, order)?;
    if let (Some(program), Some((head_order, trace))) = (&args.head, head_trace)
        && write(program, &[trace], head_order)? == 0
    {
        return Err(format!(
            "终端界面的清单是空的（{}）：第四段的{record}里一个终端界面的函数都没认出来",
            head_order.display()
        ));
    }
    Ok(())
}

/// 配着真的终端界面录第三段、`fourth` 的再录第四段（施工 LW-6，`order/head.rs`）：交回核心、终端界面各自录下的。
#[cfg(unix)]
async fn with_head(
    args: &Args,
    sandbox: &Sandbox,
    fake: &mut Fake,
    recorder: Recorder<'_>,
    trust: &Path,
    program: &Path,
    fourth: bool,
) -> Result<(Trace, Option<Trace>), String> {
    let logs = head::record(args, sandbox, fake, recorder, trust, program, fourth).await?;
    Ok((logs.core, logs.head))
}

/// 伪终端只在 Unix 上开（`order/terminal.rs`）；排函数本来也只在 Linux、macOS 上做。
#[cfg(not(unix))]
async fn with_head(
    _: &Args,
    _: &Sandbox,
    _: &mut Fake,
    _: Recorder<'_>,
    _: &Path,
    _: &Path,
    _: bool,
) -> Result<(Trace, Option<Trace>), String> {
    Err("配着终端界面录（--head）只在 Unix 上做".to_string())
}

/// 照 `program` 的符号表读几段录下的（照先后：模拟器的日志各带程序在那一段里的装载起点，探针的记录照 `order/pages.rs` 挑
/// 被测程序的那几份），清单写进 `order`，录下的文件删掉：交回清单里几个函数。
///
/// # Errors
///
/// `nm` 不在、日志读不了、探针没有记录或者记录坏了、清单写不进、录下的文件删不掉。
fn write(program: &Path, traces: &[Trace], order: &Path) -> Result<usize, String> {
    let symbols = Symbols::parse(&nm(program)?);
    let mut listing = Listing::new(&symbols);
    for trace in traces {
        match trace {
            Trace::Log(log, base) => {
                let file = std::fs::File::open(log)
                    .map_err(|e| format!("读不了 {}：{e}", log.display()))?;
                listing
                    .read(*base, BufReader::new(file))
                    .map_err(|e| format!("读不了 {}：{e}", log.display()))?;
            }
            Trace::Pages(prefix) => {
                let read = pages::read(prefix, program, &mut listing)?;
                eprintln!(
                    "排函数：探针 {}：{} 个进程的记录，{} 条，执行的 {} 条",
                    prefix.display(),
                    read.files,
                    read.records,
                    read.executed
                );
            }
        }
    }
    let names = listing.names();
    let mut text = names.join("\n");
    text.push('\n');
    std::fs::write(order, text).map_err(|e| format!("清单写不进 {}：{e}", order.display()))?;
    for trace in traces {
        match trace {
            Trace::Log(log, _) => {
                std::fs::remove_file(log).map_err(|e| format!("删不掉 {}：{e}", log.display()))?
            }
            Trace::Pages(prefix) => pages::remove(prefix)?,
        }
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
