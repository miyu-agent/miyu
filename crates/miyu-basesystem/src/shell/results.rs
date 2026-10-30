//! 跑完了交回的几句（施工 X-12 上从 `shell.rs` 分出来的那一套，施工 SB-1 挪进自己的文件）：没有输出、退出码、信号、省了多少、
//! 截了，chromium 在沙盒里起不来时多接的一句。`shell` 和脚本共用。

use std::path::Path;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::template::Template;
use miyu_tool::Done;

use super::output::{Capture, Shown};
use super::process::{Ending, Finished};
use super::{line, signal};
use crate::common::{said, said_n};
use crate::load::{self, LoadError, say};

/// 跑完了交回时的几句，`shell` 和脚本（施工 X-12 上，[`super::script`]）共用：没有输出、退出码、信号、省了多少、截了
/// （`software/basesystem/shell/`）。
pub(super) struct Results {
    pub(super) empty: Template,
    pub(super) exit: Template,
    pub(super) signal: Template,
    pub(super) omitted: Template,
    pub(super) truncated: Template,
    /// chromium 自己那层沙盒、资料目录起不来时接的一句（施工 SB-1）。
    pub(super) chromium: Template,
    /// firefox 截图写了相对路径时接的一句（施工 SB-1 修）。
    pub(super) screenshot: Template,
    /// 沙盒里用 WMI 的命令失败了接的一句（施工 SW-3）。
    pub(super) wmi: Template,
}

impl Results {
    /// 照资源目录 `resources` 里 `shell/` 下的字造。
    pub(super) fn load(resources: &Path) -> Result<Results, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, "shell", name, fields);
        Ok(Results {
            empty: text("empty", &[])?,
            exit: text("exit", &["code"])?,
            signal: text("signal", &["signal"])?,
            omitted: text("omitted", &["count"])?,
            truncated: text("truncated", &["total"])?,
            chromium: text("chromium-sandbox", &[])?,
            screenshot: text("firefox-screenshot", &[])?,
            wmi: text("no-wmi", &[])?,
        })
    }

    /// 跑完了：输出，加上它怎么结束的。到时被杀的接 `timed_out` 交的那一句和给人看的秒数（`shell`、脚本各一句）。
    pub(super) fn ended(
        &self,
        finished: Finished,
        timed_out: impl FnOnce() -> (String, String),
    ) -> Done {
        let Finished { ending, output } = finished;
        let body = self.body(&output);
        match ending {
            Ending::TimedOut => {
                let (note, seconds) = timed_out();
                Done::error(body + &note).said(said("shell/timed-out").with("seconds", seconds))
            }
            Ending::Exited(status) => match status.code() {
                Some(code) if code != 0 && chromium(&output) => {
                    let code = code.to_string();
                    let hint = say(&self.chromium, &[]);
                    Done::error(body + &say(&self.exit, &[("code", &code)]) + &hint)
                        .said(said("shell/exited").with("code", code))
                }
                None if chromium(&output) => {
                    let signal = signal(status);
                    let hint = say(&self.chromium, &[]);
                    Done::error(body + &say(&self.signal, &[("signal", &signal)]) + &hint)
                        .said(said("shell/signal").with("signal", signal))
                }
                Some(0) if output.is_empty() => {
                    Done::ok(say(&self.empty, &[])).said(said("shell/quiet"))
                }
                Some(0) => Done::ok(body).said(said_n("shell/done", "count", output.lines())),
                Some(code) => {
                    let code = code.to_string();
                    Done::error(body + &say(&self.exit, &[("code", &code)]))
                        .said(said("shell/exited").with("code", code))
                }
                None => {
                    let signal = signal(status);
                    Done::error(body + &say(&self.signal, &[("signal", &signal)]))
                        .said(said("shell/signal").with("signal", signal))
                }
            },
        }
    }

    /// 命令里是 firefox 截图、写的是相对路径的，末尾接 `firefox-screenshot.txt`（施工 SB-1 修）：firefox 起来先切到 `/`，
    /// 相对路径悄悄写不出来、退出码照样 0。不管跑成没有都接：相对路径一定写不出来。
    pub(super) fn screenshot(&self, command: &str, mut done: Done) -> Done {
        if !relative_screenshot(command) {
            return done;
        }
        append(&mut done, say(&self.screenshot, &[]));
        done
    }

    /// 在沙盒里跑（`sandboxed`）、失败了、命令里用到 WMI 的（[`WMI_COMMANDS`]），末尾接 `no-wmi.txt`（施工 SW-3，2026-10-30
    /// 项目主人定）：Windows 的沙盒（MXC 的容器）挡住 WMI，`Get-CimInstance` 一类在里面跑不成；说一句，她直接申请在沙盒外跑
    /// 这一条，不用先试错。成了的、沙盒外跑的不接。
    pub(super) fn wmi(&self, command: &str, sandboxed: bool, mut done: Done) -> Done {
        if !sandboxed || !done.error || !uses_wmi(command) {
            return done;
        }
        append(&mut done, say(&self.wmi, &[]));
        done
    }

    /// 挪到后台那一刻读到的，写进任务的输出（施工 7-12）：太长的照给她看的截成头尾两段，中间说省了多少；不说怎么看全，
    /// 读的就是全的那一份。
    pub(super) fn so_far(&self, output: &Capture) -> String {
        match output.shown() {
            Shown::Whole(text) => text,
            Shown::Cut {
                head,
                tail,
                omitted,
                ..
            } => line(head) + &say(&self.omitted, &[("count", &omitted.to_string())]) + &tail,
        }
    }

    /// 给她看的输出：太长的截成头尾两段，中间说省了多少，末尾说一共多少、怎么看全。每一段都以换行结尾。
    pub(super) fn body(&self, output: &Capture) -> String {
        match output.shown() {
            Shown::Whole(text) => line(text),
            Shown::Cut {
                head,
                tail,
                omitted,
                total,
            } => {
                let omitted = say(&self.omitted, &[("count", &omitted.to_string())]);
                let truncated = say(&self.truncated, &[("total", &total.to_string())]);
                line(head) + &omitted + &line(tail) + &truncated
            }
        }
    }
}

/// chromium 自己那层沙盒、资料目录在 Miyu 的沙盒里起不来时它自己说的几句（施工 SB-1，2026-10-23 项目主人定，同 Claude Code
/// 的说法：外层沙盒还在，内层多余）：失败的结果里认出一句，末尾接 `chromium-sandbox.txt`，教她加 `--no-sandbox
/// --user-data-dir=<临时目录>`。只有撞上才花 token，不进 `shell` 的说明。只认原文里固定的那一截。
///
/// 施工 SB-1 修二（主人验收撞见）：加了 `--no-sandbox --user-data-dir` 以后，crashpad 照 `XDG_CONFIG_HOME` 在
/// `~/.config/chromium/Crash Reports` 建库，那里被沙盒藏成只读的空目录，建不了就把主进程一起带崩（SIGTRAP）。认它的两种
/// 说法（随系统语言变的那半不认），提示多教一个 `XDG_CONFIG_HOME=<临时目录>`。
/// 末尾一块字后面接 `hint`；一块字都没有的另起一块。
fn append(done: &mut Done, hint: String) {
    match done.blocks.iter_mut().rev().find_map(|block| match block {
        Block::Text(text) => Some(text),
        _ => None,
    }) {
        Some(text) => text.text.push_str(&hint),
        None => done.blocks.push(Block::Text(Text { text: hint })),
    }
}

/// 要连 WMI 的命令（小写，施工 SW-3）：CIM、WMI 的 cmdlet 和它们的别名，`wmic`，底下走 WMI 的 `systeminfo`、
/// `Get-ComputerInfo`。
const WMI_COMMANDS: [&str; 8] = [
    "get-ciminstance",
    "gcim",
    "get-wmiobject",
    "gwmi",
    "invoke-cimmethod",
    "wmic",
    "systeminfo",
    "get-computerinfo",
];

/// 命令里有一个词（去掉路径、`.exe`、引号）是 [`WMI_COMMANDS`] 之一，大小写不论。
fn uses_wmi(command: &str) -> bool {
    command
        .split(|c: char| c.is_whitespace() || matches!(c, ';' | '|' | '&' | '(' | ')' | '{' | '}'))
        .map(|word| {
            let word = word
                .trim_matches(|c| c == '\'' || c == '"')
                .to_ascii_lowercase();
            let name = word
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or_default()
                .to_string();
            name.strip_suffix(".exe")
                .map(str::to_string)
                .unwrap_or(name)
        })
        .any(|name| WMI_COMMANDS.contains(&name.as_str()))
}

const CHROMIUM_SIGNS: [&str; 6] = [
    "Failed to create a unique user data directory",
    "The SUID sandbox helper binary was found, but is not configured correctly",
    "Zygote process exited prematurely",
    "No usable sandbox!",
    "chrome_crashpad_handler: --database is required",
    "Crash Reports",
];

/// 输出里有没有 chromium 那几句（截了的看头尾两段）。
fn chromium(output: &Capture) -> bool {
    let says = |text: &str| CHROMIUM_SIGNS.iter().any(|sign| text.contains(sign));
    match output.shown() {
        Shown::Whole(text) => says(&text),
        Shown::Cut { head, tail, .. } => says(&head) || says(&tail),
    }
}

/// 命令里调了 firefox、`--screenshot` 写的是相对路径（施工 SB-1 修）：路径跟在 `=` 后面或者是下一个词，去掉引号以后不以
/// `/`、`$`、`~` 开头的算相对。没写路径的（下一个词是网址、别的参数，或者没有了）是默认的 `screenshot.png`，也是相对的：
/// 下一个词不管是什么，照同一条规矩判就对了。
fn relative_screenshot(command: &str) -> bool {
    let words: Vec<&str> = command.split_whitespace().collect();
    let firefox = words.iter().any(|word| {
        let name = word.rsplit('/').next().unwrap_or(word);
        name.starts_with("firefox")
    });
    if !firefox {
        return false;
    }
    let path =
        words
            .iter()
            .enumerate()
            .find_map(|(at, word)| match word.strip_prefix("--screenshot=") {
                Some(path) => Some(path),
                None => (*word == "--screenshot")
                    .then(|| words.get(at + 1).copied().unwrap_or_default()),
            });
    path.is_some_and(|path| {
        let path = path.trim_start_matches(['"', '\'']);
        !(path.starts_with('/') || path.starts_with('$') || path.starts_with('~'))
    })
}
