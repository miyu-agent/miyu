//! 跑完了交回的几句（施工 X-12 上从 `shell.rs` 分出来的那一套，施工 SB-1 挪进自己的文件）：没有输出、退出码、信号、省了多少、
//! 截了，chromium 在沙盒里起不来时多接的一句。`shell` 和脚本共用。

use std::path::Path;

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
const CHROMIUM_SIGNS: [&str; 4] = [
    "Failed to create a unique user data directory",
    "The SUID sandbox helper binary was found, but is not configured correctly",
    "Zygote process exited prematurely",
    "No usable sandbox!",
];

/// 输出里有没有 chromium 那几句（截了的看头尾两段）。
fn chromium(output: &Capture) -> bool {
    let says = |text: &str| CHROMIUM_SIGNS.iter().any(|sign| text.contains(sign));
    match output.shown() {
        Shown::Whole(text) => says(&text),
        Shown::Cut { head, tail, .. } => says(&head) || says(&tail),
    }
}
