//! 给人看的报错、警告、提示（设计 47 第三节）：报错 `miyu: ` 打头，警告在它后面多一个 `warning:`（中文「警告：」），建议另起
//! 一行写 `hint:`（中文「提示：」），照 GNU 程序和它们的中文翻译。命令行往标准错误上印这三种，都经这里。

#[cfg(test)]
mod tests;

use std::io::{self, Write};

use crate::exit;
use crate::language::Language;
use crate::shown::say;

/// 报错、警告打头的程序名。
const PROGRAM: &str = "miyu";

/// 一件事：它是什么（一行），能照着做的几条建议（一条一行）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Problem {
    what: String,
    hints: Vec<String>,
}

impl Problem {
    /// 出了 `what` 这件事。别处交来的原话（核心的拒绝、底层的错误）照规矩收拾：去掉句末的句号，英文开头的大写词改小写
    /// （`Owner only.` 成 `owner only`），全大写的缩写不动（`MCP`）。
    pub(crate) fn new(what: impl Into<String>) -> Problem {
        Problem {
            what: tidy(&what.into()),
            hints: Vec::new(),
        }
    }

    /// 再给一条建议：能照着敲的命令，或者一句操作。
    pub(crate) fn hint(mut self, hint: impl Into<String>) -> Problem {
        self.hints.push(tidy(&hint.into()));
        self
    }

    /// 写成几行：报错是 `miyu: <事>`，警告是 `miyu: warning: <事>`，后面每条建议一行。
    pub(crate) fn lines(&self, language: Language, level: Level) -> Vec<String> {
        let first = match (level, language) {
            (Level::Error, _) => format!("{PROGRAM}: {}", self.what),
            (Level::Warning, Language::Chinese) => format!("{PROGRAM}: 警告：{}", self.what),
            (Level::Warning, Language::English) => format!("{PROGRAM}: warning: {}", self.what),
        };
        let hint = match language {
            Language::Chinese => "提示：",
            Language::English => "hint: ",
        };
        std::iter::once(first)
            .chain(self.hints.iter().map(|one| format!("{hint}{one}")))
            .collect()
    }
}

/// 报错还是警告。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Level {
    /// 没做成。
    Error,
    /// 照样做了，提醒一句。
    Warning,
}

/// 往 `to` 上印一条报错。
pub(crate) fn error(to: &mut dyn Write, language: Language, problem: &Problem) {
    for line in problem.lines(language, Level::Error) {
        say(to, &line);
    }
}

/// 往 `to` 上印一条警告。
pub(crate) fn warning(to: &mut dyn Write, language: Language, problem: &Problem) {
    for line in problem.lines(language, Level::Warning) {
        say(to, &line);
    }
}

/// 在标准错误上报错，交回出错的退出码：连上核心以前就出了错的那几处都走它。
pub(crate) fn failed(language: Language, problem: &Problem) -> u8 {
    error(&mut io::stderr(), language, problem);
    exit::ERROR
}

/// 在标准错误上报一件没有建议的错，交回出错的退出码：原来十四个模块各抄一份的 `failed`。没有建议，不用分语言。
pub(crate) fn fail(reason: impl Into<String>) -> u8 {
    say(
        &mut io::stderr(),
        &format!("{PROGRAM}: {}", tidy(&reason.into())),
    );
    exit::ERROR
}

/// 收拾一句话：去掉两头的空白和句末的句号，开头是首字母大写的英文词的改成小写。
fn tidy(text: &str) -> String {
    let text = text.trim().trim_end_matches(['。', '.']).trim_end();
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(first), Some(second))
            if first.is_ascii_uppercase() && second.is_ascii_lowercase() =>
        {
            let mut lowered = first.to_ascii_lowercase().to_string();
            lowered.push_str(&text[first.len_utf8()..]);
            lowered
        }
        _ => text.to_string(),
    }
}
