//! 配着真的终端界面录的两段（施工 LW-6，`internal/blueprint/perf.md`「排函数」）：`miyu-perf --order` 带了 `--head`、
//! `--head-order` 的，在照协议问的两段以后接着录。
//!
//! - 第三段：核心在模拟器里，真的终端界面原生。原生起主程序 `miyu`（不带参数），它连上正在运行的核心、换成终端界面；
//!   核心这一段执行到的函数接在核心清单后面。
//! - 第四段：核心原生，终端界面放进模拟器（`<模拟器> -d in_asm -D <日志> <终端界面>`），照第三段看到的命令行参数、主程序给
//!   它加的环境变量起（[`Launched`]）；这一段的日志是终端界面自己的清单。
//!
//! 两段的剧本一样（[`converse`]）：等输入框画出来（屏幕上出现 [`PROMPT`]），说一句，等假模型收到这一轮的主请求、屏幕上
//! 画出回答、这一轮结束；再说一句触发工具调用的话（工具一步的几种各调一次），等这一轮结束；歇 [`QUIET`]。哪一样等不到都
//! 报错停下，不悄悄少录。
//!
//! 沙盒：配置加上 `[ui] welcomed = true`、`[tui] auto_pick = true`，跳过欢迎和选人格（没人点）；家目录、`XDG_*` 都指进
//! 量尺的工作目录（[`home_vars`]），终端界面和核心都不碰真的家目录。都不设 `XDG_RUNTIME_DIR`：核心、主程序、终端界面照
//! 数据根里的 `run/` 找套接字，和量尺拉起的核心是同一处。

use std::collections::HashSet;
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use super::script::QUIET;
use super::terminal::Terminal;
use crate::args::Args;
use crate::fake::script::{Script, ToolCall};
use crate::fake::{Arrival, Fake, mentions};
use crate::measure::tools::calls::{KINDS, Workspace};
use crate::sandbox::{Launch, Sandbox, Traced, base_of, home_vars};

/// 等终端界面最多多久：核心或者终端界面在模拟器里，头一回执行每一块代码都要先翻译，比原生慢十几倍（同
/// `sandbox.rs` 的 `TRACED_PATIENCE`）。
const PATIENCE: Duration = Duration::from_secs(600);

/// 终端界面画好输入框的记号：输入框前面的提示符。
const PROMPT: &str = "❯";

/// 屏幕上画出回答的记号：假模型的回答（[`crate::fake::filler`]）里反复出现的一段英文，不会被宽字符劈开。
const ANSWERED: &str = "measuring reply";

/// 主请求才带工具表：起标题、记忆这些辅助请求也提到这一句的记号，不算。
const TOOLS: &[u8] = b"\"tools\"";

/// 跳过欢迎、自动选人格：沙盒里没人点。
const SETTINGS: &str = "\n[ui]\nwelcomed = true\n\n[tui]\nauto_pick = true\n";

/// 打字以前屏幕要安静多久（[`send`]）。
const SETTLE: Duration = Duration::from_secs(1);

/// 等屏幕安静最多等多久：一直在画的（转圈的记号）到点照样往下打字，字丢了由后面等它出现在输入框里的那一步报出来。
const SETTLE_CAP: Duration = Duration::from_secs(30);

/// 多久看一次会话日志里这一轮结束了没有。
const POLL: Duration = Duration::from_millis(200);

/// 录好的日志：核心的（第三段）、终端界面的（第四段，录了的话），各带程序在那一段里的装载起点。
pub struct Logs {
    /// 第三段：核心的日志和装载起点。
    pub core: (PathBuf, u64),
    /// 第四段：终端界面的日志和装载起点；没录的是 `None`。
    pub head: Option<(PathBuf, u64)>,
}

/// 录第三段，`fourth` 的再录第四段：终端界面是 `program`，核心在模拟器 `emulator` 里，核心信任 `trust`，工具一步的工作区
/// 拷 `--tree`。
///
/// # Errors
///
/// 哪一步没成：沙盒配不了、核心或终端界面起不来、等不到输入框、等不到回答、工具调用没做完。
pub async fn record(
    args: &Args,
    sandbox: &Sandbox,
    fake: &mut Fake,
    emulator: &Path,
    trust: &Path,
    program: &Path,
    fourth: bool,
) -> Result<Logs, String> {
    let tree = args.tree.as_deref().ok_or("排函数要 --tree")?;
    let home = args.work.join("user");
    let distro = home.join("distro");
    std::fs::create_dir_all(&distro).map_err(|e| format!("建不了 {}：{e}", distro.display()))?;
    let config = sandbox.root.config_file();
    let mut text = std::fs::read_to_string(&config).map_err(|e| format!("系统配置读不了：{e}"))?;
    text.push_str(SETTINGS);
    std::fs::write(&config, text).map_err(|e| format!("系统配置写不进：{e}"))?;
    let given = environment(sandbox, args, trust, &home);
    let native = Launch {
        trust: Some(trust),
        home: Some(&home),
        ..Launch::default()
    };

    eprintln!("排函数：第三段（核心在模拟器里，真的终端界面）");
    let log = args.work.join("order-3.log");
    let workspace = Workspace::prepare(&sandbox.work.join("tools-3"), tree)?;
    let running = sandbox.start_with(Launch {
        traced: Some(Traced {
            emulator,
            log: &log,
        }),
        ..native
    })?;
    let base = running.base(&args.miyu);
    let mut command = Command::new(&args.miyu);
    command
        .current_dir(&workspace.dir)
        .env_clear()
        .envs(given.iter().cloned());
    let turns = Turns::new(sandbox)?;
    let third = async {
        let mut terminal = Terminal::spawn(command)?;
        terminal
            .wait_for(PROMPT, 0, PATIENCE, "主程序拉起终端界面、画出输入框")
            .await?;
        let launched = Launched::read(terminal.pid(), program, &given)?;
        converse(&mut terminal, fake, &turns, &workspace, "perf-order-head-3").await?;
        tokio::time::sleep(QUIET).await;
        Ok::<_, String>(launched)
    }
    .await;
    sandbox.stop(running)?;
    let core = (log, base?);
    let launched = third.map_err(|e| format!("第三段：{e}"))?;
    if !fourth {
        return Ok(Logs { core, head: None });
    }

    eprintln!("排函数：第四段（终端界面在模拟器里）");
    let log = args.work.join("order-4.log");
    let workspace = Workspace::prepare(&sandbox.work.join("tools-4"), tree)?;
    let running = sandbox.start_with(native)?;
    let mut command = Command::new(emulator);
    command
        .current_dir(&workspace.dir)
        .args(["-d", "in_asm", "-D"])
        .arg(&log)
        .arg(program)
        .args(&launched.args)
        .env_clear()
        .envs(given.iter().cloned())
        .envs(launched.extra.iter().cloned());
    let turns = Turns::new(sandbox)?;
    let recorded = async {
        let mut terminal = Terminal::spawn(command)?;
        terminal
            .wait_for(PROMPT, 0, PATIENCE, "终端界面在模拟器里画出输入框")
            .await?;
        let base = base_of(terminal.pid(), program)?;
        converse(&mut terminal, fake, &turns, &workspace, "perf-order-head-4").await?;
        tokio::time::sleep(QUIET).await;
        Ok::<_, String>(base)
    }
    .await;
    sandbox.stop(running)?;
    let base = recorded.map_err(|e| format!("第四段：{e}"))?;
    Ok(Logs {
        core,
        head: Some((log, base)),
    })
}

/// 给主程序、终端界面的环境（整份，不接宿主的）：宿主的 `PATH`、`LANG`、`LC_*`；终端是 `xterm-256color`、真彩色；家目录
/// 照 [`home_vars`]，缓存在家目录里；数据根、资源、证书和量尺拉起的核心一样；不拉 models.dev 的目录，不走代理。
fn environment(
    sandbox: &Sandbox,
    args: &Args,
    trust: &Path,
    home: &Path,
) -> Vec<(OsString, OsString)> {
    let mut vars: Vec<(OsString, OsString)> = std::env::vars_os()
        .filter(|(name, _)| {
            let name = name.as_bytes();
            name == b"PATH" || name == b"LANG" || name.starts_with(b"LC_")
        })
        .collect();
    let set = [
        ("TERM", OsString::from("xterm-256color")),
        ("COLORTERM", OsString::from("truecolor")),
        ("XDG_CACHE_HOME", home.join(".cache").into_os_string()),
        ("MIYU_HOME", sandbox.root.path().as_os_str().to_os_string()),
        ("MIYU_RESOURCES", args.resources.clone().into_os_string()),
        ("MIYU_CATALOG_UPDATE", OsString::from("false")),
        ("SSL_CERT_FILE", trust.as_os_str().to_os_string()),
        ("NO_PROXY", OsString::from("127.0.0.1")),
    ];
    let homes = home_vars(home).map(|(name, path)| (name, path.into_os_string()));
    vars.extend(
        set.into_iter()
            .chain(homes)
            .map(|(name, value)| (OsString::from(name), value)),
    );
    vars
}

/// 主程序是怎么拉起终端界面的（第三段原生看到的）：第四段照着在模拟器里起。
#[derive(Debug, PartialEq, Eq)]
pub struct Launched {
    /// 程序名以后的命令行参数。
    pub args: Vec<OsString>,
    /// 主程序给它加的、改了的环境变量（`given` 里没有这一对的）。
    pub extra: Vec<(OsString, OsString)>,
}

impl Launched {
    /// 读进程 `pid`（主程序已经换成了终端界面）的 `/proc/<进程号>/exe`、`cmdline`、`environ`。主程序起的要是 `program`：
    /// 不是的说明发行包目录里的终端界面和 `--head` 给的不是同一个，两段录的不是同一个程序。
    ///
    /// # Errors
    ///
    /// 读不了；起的不是 `program`。
    fn read(pid: u32, program: &Path, given: &[(OsString, OsString)]) -> Result<Launched, String> {
        let proc = PathBuf::from(format!("/proc/{pid}"));
        let read = |name: &str| {
            std::fs::read(proc.join(name))
                .map_err(|e| format!("读不了 {}：{e}", proc.join(name).display()))
        };
        let exe = std::fs::read_link(proc.join("exe"))
            .map_err(|e| format!("读不了 {}：{e}", proc.join("exe").display()))?;
        let wanted = program
            .canonicalize()
            .map_err(|e| format!("找不到 {}：{e}", program.display()))?;
        if exe != wanted {
            return Err(format!(
                "主程序拉起的是 {}，不是 --head 给的 {}：发行包目录里的终端界面要和 --head 是同一个",
                exe.display(),
                wanted.display()
            ));
        }
        let args = nul_list(&read("cmdline")?).into_iter().skip(1).collect();
        Ok(Launched {
            args,
            extra: extra(&nul_list(&read("environ")?), given),
        })
    }
}

/// `/proc` 里用 NUL 隔开、末尾带一个 NUL 的一串（`cmdline`、`environ`）。空的参数照样留着。
fn nul_list(bytes: &[u8]) -> Vec<OsString> {
    let bytes = bytes.strip_suffix(&[0]).unwrap_or(bytes);
    if bytes.is_empty() {
        return Vec::new();
    }
    bytes
        .split(|&byte| byte == 0)
        .map(|part| OsString::from_vec(part.to_vec()))
        .collect()
}

/// `environ` 里 `given` 没有的那几对（名字、值都一样才算有）。没有 `=` 的不算。
fn extra(environ: &[OsString], given: &[(OsString, OsString)]) -> Vec<(OsString, OsString)> {
    environ
        .iter()
        .filter_map(|entry| {
            let bytes = entry.as_bytes();
            let at = bytes.iter().position(|&byte| byte == b'=')?;
            let pair = (
                OsString::from_vec(bytes[..at].to_vec()),
                OsString::from_vec(bytes[at + 1..].to_vec()),
            );
            (!given.contains(&pair)).then_some(pair)
        })
        .collect()
}

/// 剧本（两段一样）：说一句等回答，再说一句做完工具一步的几种（只做工作区这一级的：终端界面开的会话就在这一级），记号
/// 照 `tag`。
///
/// # Errors
///
/// 打的字没出现在输入框里、模型没收到、屏幕上没画出回答、这一轮没结束、工具调用的先后对不上。
async fn converse(
    terminal: &mut Terminal,
    fake: &mut Fake,
    turns: &Turns<'_>,
    workspace: &Workspace,
    tag: &str,
) -> Result<(), String> {
    let marker = format!("{tag}-say");
    let before = turns.ended()?;
    let typed = send(terminal, &format!("{marker}: keep going."), &marker).await?;
    main_request(fake, &marker)
        .await
        .map_err(|e| format!("说的那一句：{e}"))?;
    terminal
        .wait_for(ANSWERED, typed, PATIENCE, "说的那一句，屏幕上画出回答")
        .await?;
    turns.over(before, "说的那一句").await?;

    let marker = format!("{tag}-tools");
    let calls: Vec<ToolCall> = KINDS
        .iter()
        .filter(|kind| !kind.full)
        .enumerate()
        .map(|(n, kind)| ToolCall {
            id: format!("perf_head_{tag}_{n:02}"),
            name: kind.tool.to_string(),
            arguments: kind.args(workspace, 0).to_string(),
        })
        .collect();
    let count = calls.len();
    fake.script(Some(Script {
        marker: marker.clone(),
        calls,
    }));
    let before = turns.ended()?;
    send(terminal, &format!("{marker}: go."), &marker).await?;
    for expected in 0..=count {
        let arrival = main_request(fake, &marker)
            .await
            .map_err(|e| format!("工具调用那一句，等第 {expected} 个请求：{e}"))?;
        if arrival.step != Some(expected) {
            return Err(format!(
                "工具调用那一句：第 {expected} 个请求带着的结果对不上（{:?}），一共 {count} 次",
                arrival.step
            ));
        }
    }
    turns.over(before, "工具调用那一句").await?;
    fake.script(None);
    Ok(())
}

/// 往输入框里打 `text`，等它出现在屏幕上（`marker`）再回车：交回回车以前屏幕记到哪，之后画的从这里找。打字以前先等屏幕
/// 安静下来（[`SETTLE`]）：终端界面画出输入框的那一刻还没开始读按键，这时打的字会丢（实测原生起来以后立刻打就丢了，
/// 隔 0.3 秒就不丢；模拟器里慢十几倍）。
async fn send(terminal: &mut Terminal, text: &str, marker: &str) -> Result<usize, String> {
    terminal.quiet(SETTLE, SETTLE_CAP).await;
    let mark = terminal.mark();
    terminal.write(text.as_bytes())?;
    terminal
        .wait_for(marker, mark, PATIENCE, "打的字出现在输入框里")
        .await?;
    let mark = terminal.mark();
    terminal.write(b"\r")?;
    Ok(mark)
}

/// 等这一轮的主请求：提到 `marker`、带工具表的那一个（[`TOOLS`]）。
async fn main_request(fake: &mut Fake, marker: &str) -> Result<Arrival, String> {
    loop {
        let arrival = fake.request_with(marker).await?;
        if mentions(&arrival.body, TOOLS) {
            return Ok(arrival);
        }
    }
}

/// 数这一段的会话里结束了几轮：终端界面开的会话日志里的 `turn.ended`。这一段起终端界面以前就有的会话（前几段的）不看：
/// 不白读，也不会被它们的回合数混进来。
struct Turns<'a> {
    sandbox: &'a Sandbox,
    old: HashSet<String>,
}

impl<'a> Turns<'a> {
    /// 记下现在已经有的会话。起终端界面以前调。
    ///
    /// # Errors
    ///
    /// 会话的目录读不了。
    fn new(sandbox: &'a Sandbox) -> Result<Turns<'a>, String> {
        let old = sessions(sandbox)?
            .iter()
            .map(|id| id.as_str().to_string())
            .collect();
        Ok(Turns { sandbox, old })
    }

    /// 一共几条 `turn.ended`。
    ///
    /// # Errors
    ///
    /// 会话的目录读不了。
    fn ended(&self) -> Result<usize, String> {
        Ok(self.count()?.0)
    }

    /// [`Turns::ended`]，另交回读不出来的会话和原因（刚建、还没写下一段的读不出来，当它还没有回合）：等不到的时候附在
    /// 报错后面。
    fn count(&self) -> Result<(usize, Vec<String>), String> {
        let root = &self.sandbox.root;
        let mut total = 0;
        let mut unread = Vec::new();
        for id in sessions(self.sandbox)? {
            if self.old.contains(id.as_str()) {
                continue;
            }
            match miyu_store::log::read_events(&root.session_dir(&id)) {
                Ok(events) => {
                    total += events
                        .iter()
                        .filter(|event| event.body.kind() == "turn.ended")
                        .count();
                }
                Err(e) => unread.push(format!("{}：{e}", id.as_str())),
            }
        }
        Ok((total, unread))
    }

    /// 等 `turn.ended` 比 `before` 多：这一轮结束了（`what` 是哪一句）。
    ///
    /// # Errors
    ///
    /// 等太久、会话的目录读不了。
    async fn over(&self, before: usize, what: &str) -> Result<(), String> {
        let began = Instant::now();
        loop {
            let (total, unread) = self.count()?;
            if total > before {
                return Ok(());
            }
            if began.elapsed() > PATIENCE {
                return Err(format!(
                    "{what}：等了 {} 秒这一轮没结束（终端界面开的会话里 turn.ended 还是 {before} 条；读不出来的会话：{unread:?}）",
                    PATIENCE.as_secs()
                ));
            }
            tokio::time::sleep(POLL).await;
        }
    }
}

/// 沙盒里的会话。
fn sessions(sandbox: &Sandbox) -> Result<Vec<miyu_kernel::id::SessionId>, String> {
    sandbox
        .root
        .sessions()
        .map_err(|e| format!("会话的目录读不了：{e}"))
}

#[cfg(test)]
mod tests;
