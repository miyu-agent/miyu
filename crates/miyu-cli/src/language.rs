//! 给人看的话跟着界面语言（`docs/designs/22-命令行.md` 第二节）：握手以前照系统的语言，`zh` 开头的说中文，别的说
//! 英文；握手以后照核心回的 `language`，它照这个人的 `ui.language` 算（施工 8-2）。界面的字先写在这里，做界面
//! 语言的那一步挪进资源文件（`00-设计理念.md` 第六节）。

use crate::ask::usage_line;
use crate::diagnostic::Problem;

mod agents;
mod config;
mod config_write;
mod confirm;
mod embedding;
mod harness;
mod input;
mod kb;
mod login;
mod mcp;
mod memory;
pub use memory::Organized;
mod pkg;
mod sandbox;
mod schedule;
mod service;
mod setup;
mod undo;

pub(crate) use confirm::{Carried, Doing, PlanLabel};
pub(crate) use pkg::{CameFrom, Checked, InfoKey, LogoWhy};

/// 界面语言。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// 中文。
    Chinese,
    /// 英文。
    English,
}

/// 一步的结果里头自己写的几个词（施工 4-5 下）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Word {
    /// 出错了：标红，有原因的后面跟原因。
    Failed,
    /// 被拒了，没做：标红，有原因的后面跟原因。
    Denied,
    /// 打断了：没有说法时写。
    Cancelled,
    /// 跳过了：没有说法时写。
    Skipped,
}

/// 握手以前的界面语言：照系统的语言（`miyu_store::env::locale`：`LC_ALL`、`LC_MESSAGES`、`LANG`，都没设的看系统设置，
/// 施工 8-2），`zh` 开头的说中文，别的说英文。握手以后照核心回的 `language`（[`Language::from_code`]）。
pub fn current() -> Language {
    of_locale(miyu_store::env::locale().as_deref())
}

/// 照系统的语言 `locale` 挑：`zh` 开头的是中文，别的、没有的是英文。
pub fn of_locale(locale: Option<&str>) -> Language {
    match locale {
        Some(value) if value.starts_with("zh") => Language::Chinese,
        _ => Language::English,
    }
}

impl Language {
    /// 核心握手时回的 `language`（`zh`、`en`、`ja`）换成命令行的界面语言（施工 8-2）：命令行自己的字只有中文、英文，
    /// `ja` 的照英文。认不出的是空的。
    pub fn from_code(code: &str) -> Option<Language> {
        match code {
            "zh" => Some(Language::Chinese),
            "en" | "ja" => Some(Language::English),
            _ => None,
        }
    }

    /// 握手时报给核心的语言：核心的拒绝照它说。
    pub fn locale(&self) -> &'static str {
        match self {
            Language::Chinese => "zh-CN",
            Language::English => "en",
        }
    }

    /// 给人看的字读哪一份：`human/<它>.json`（施工 4-5 下）。
    pub fn code(&self) -> &'static str {
        match self {
            Language::Chinese => "zh",
            Language::English => "en",
        }
    }

    /// 一步的结果里头自己写的词。
    pub(crate) fn word(&self, word: Word) -> &'static str {
        let (chinese, english) = match word {
            Word::Failed => ("出错", "failed"),
            Word::Denied => ("没做", "not done"),
            Word::Cancelled => ("打断了", "interrupted"),
            Word::Skipped => ("已跳过", "skipped"),
        };
        match self {
            Language::Chinese => chinese,
            Language::English => english,
        }
    }

    /// 有 `steps` 步因为要确认没做的告警（`22-命令行.md` O3，施工 4-9，设计 47）。
    pub(crate) fn unattended(&self, steps: u64) -> Problem {
        match (self, steps) {
            (Language::Chinese, _) => {
                Problem::new(format!("{steps} 步需要确认，已跳过")).hint("在终端界面中运行可以确认")
            }
            (Language::English, 1) => Problem::new("1 step needs approval and was skipped")
                .hint("run it in the terminal interface to approve"),
            (Language::English, _) => {
                Problem::new(format!("{steps} steps need approval and were skipped"))
                    .hint("run it in the terminal interface to approve")
            }
        }
    }

    /// 「出错」「没做」和原因之间。
    pub(crate) fn colon(&self) -> &'static str {
        match self {
            Language::Chinese => "：",
            Language::English => ": ",
        }
    }

    /// 接着说的会话在别处干活（施工 9-7 上）：工作区是会话的，`used` 是它在的目录。过程记录里的一行。
    pub(crate) fn elsewhere(&self, used: &str) -> String {
        match self {
            Language::Chinese => format!("· 会话的工作目录：{used}"),
            Language::English => format!("· Session working directory: {used}"),
        }
    }

    /// 工作目录太宽、核心退回了数据根的工作区的告警（设计 47）：`given` 是敲命令时的目录，`used` 是实际干活的。
    pub(crate) fn moved(&self, given: &str, used: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("工作目录 {given} 范围过大，改用 {used}")),
            Language::English => Problem::new(format!(
                "working directory {given} is too broad; using {used}"
            )),
        }
    }

    /// 没有这个子命令（设计 47）。
    pub(crate) fn no_such_command(&self, name: &str) -> Problem {
        match self {
            Language::Chinese => {
                Problem::new(format!("未知命令 '{name}'")).hint("运行 'miyu --help' 查看可用命令")
            }
            Language::English => {
                Problem::new(format!("unknown command '{name}'")).hint("see 'miyu --help'")
            }
        }
    }

    /// 不认识的命令是一个没装的软件包带的（施工 CL-7）：说去装它。
    pub(crate) fn command_not_installed(&self, name: &str, package: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("未知命令 '{name}'")).hint(format!(
                "软件包 {package} 提供这个命令，用 'miyu pkg install {package}' 安装"
            )),
            Language::English => Problem::new(format!("unknown command '{name}'")).hint(format!(
                "package {package} provides it; install it with 'miyu pkg install {package}'"
            )),
        }
    }

    /// 没有可用的模型（施工 8-6；施工 8-11 起指向 `miyu setup`，`models.md`「给人看的字」）。
    pub(crate) fn no_model(&self) -> Problem {
        match self {
            Language::Chinese => Problem::new("未配置模型").hint("运行 'miyu setup' 添加模型"),
            Language::English => {
                Problem::new("no model configured").hint("run 'miyu setup' to add one")
            }
        }
    }

    /// `--continue` 找不到可以接着说的会话。
    pub(crate) fn no_oneshot(&self) -> Problem {
        match self {
            Language::Chinese => Problem::new("没有可继续的 miyu ask 会话"),
            Language::English => Problem::new("no session from 'miyu ask' to continue"),
        }
    }

    /// 出错了：哪一类，原话。
    pub(crate) fn failed(&self, class: &str, message: &str) -> Problem {
        let (kind, said) = (self.class(class), message.trim());
        match (self, said.is_empty()) {
            (_, true) => Problem::new(kind),
            (Language::Chinese, false) => Problem::new(format!("{kind}：{said}")),
            (Language::English, false) => Problem::new(format!("{kind}: {said}")),
        }
    }

    /// 出错的分类怎么说（`model.called` 的 `error.class`）。
    /// 出错的分类说成给人看的（施工 6-3 下：压缩失败那一行也用）。
    pub(crate) fn class_name(&self, class: &str) -> &'static str {
        self.class(class)
    }

    fn class(&self, class: &str) -> &'static str {
        let (chinese, english) = match class {
            "retryable" => ("临时错误", "temporary error"),
            "rate_limited" => ("请求受限", "rate limited"),
            "context_too_long" => ("上下文太长", "context too long"),
            "auth" => ("认证失败", "authentication failed"),
            "quota" => ("额度用完", "out of quota"),
            "content_policy" => ("内容被拦截", "blocked by content policy"),
            "bad_stream" => ("响应流异常", "malformed response stream"),
            "empty_reply" => ("响应为空", "empty response"),
            "bad_summary" => ("摘要提取失败", "summary extraction failed"),
            "compaction_paused" => ("自动压缩已暂停", "auto-compaction paused"),
            "no_model" => ("没有可用的模型", "no model available"),
            "cooling" => ("模型冷却中", "models cooling down"),
            "output_limit" => ("回复超过输出上限", "reply exceeded the output limit"),
            _ => ("模型错误", "model error"),
        };
        match self {
            Language::Chinese => chinese,
            Language::English => english,
        }
    }

    /// 被打断了。
    pub(crate) fn interrupted(&self) -> Problem {
        match self {
            Language::Chinese => Problem::new("已中断"),
            Language::English => Problem::new("interrupted"),
        }
    }

    /// 这一轮没走完：步数用完、核心崩了或者重启了。
    pub(crate) fn unfinished(&self, reason: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("本轮未完成：{reason}")),
            Language::English => Problem::new(format!("turn did not finish: {reason}")),
        }
    }

    /// 核心断开了。
    pub(crate) fn disconnected(&self) -> Problem {
        match self {
            Language::Chinese => Problem::new("与核心的连接已断开"),
            Language::English => Problem::new("lost connection to the core"),
        }
    }

    /// 附件传不上（施工 3-9 三补）：哪个文件，核心照握手时报的语言说的原因。
    pub(crate) fn not_attached(&self, file: &str, reason: &str) -> Problem {
        let reason = reason.trim_end_matches(['。', '.']);
        match self {
            Language::Chinese => Problem::new(format!("无法附加 {file}：{reason}")),
            Language::English => Problem::new(format!("cannot attach {file}: {reason}")),
        }
    }

    /// 核心拒绝了：它的原话已经照握手时报的语言说了，照报错的规矩收拾（设计 47）。
    pub(crate) fn refused(&self, reason: &str) -> Problem {
        Problem::new(reason)
    }

    /// 核心拒了，原话 `said` 后面接上它给的为什么 `why`（`data.message`：知识库挂不上的目录、挪不进回收站，整理记忆没成，
    /// 施工 K-7 下、R-7 修）。为什么是核心给的英文一句，照原样接。
    pub(crate) fn refused_because(&self, said: &str, why: &str) -> Problem {
        let said = said.trim_end_matches(['。', '.']);
        match self {
            Language::Chinese => Problem::new(format!("{said}：{why}")),
            Language::English => Problem::new(format!("{said}: {why}")),
        }
    }

    /// 问完那一行用量。
    pub(crate) fn usage(&self, sum: &crate::ask::Sum) -> String {
        usage_line(self, sum)
    }
}
