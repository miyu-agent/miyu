//! 要说的话从哪来（施工 AP-2，设计 50 第二节）：命令行上写的几个词，标准输入。没写消息、标准输入不是终端的，读到头，整段
//! 就是消息；有一个词正好是 `-` 的，去掉它，读标准输入接在消息后面，空一行隔开；写了消息、没写 `-` 的不读：别的程序拉起
//! `miyu` 时常常留着一个不关的管道，一看到管道就读会一直等下去（同 `codex exec` 的 `-`）。

use std::io::{self, Read};

use super::exit;
use crate::diagnostic;
use crate::language::Language;

/// 凑不出要说的话：为什么。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Unread {
    /// 没有消息：没写、终端里没写，读到的是空的、只有空白的。照参数写错报，退出码 2。
    Missing,
    /// 读到的不是 UTF-8。照参数写错报，退出码 2。
    NotUtf8,
    /// 读标准输入出了错：底层的原话。退出码 1。
    Failed(String),
}

/// 照命令行上的 `words` 和标准输入 `stdin` 凑出要说的话；`terminal` 是标准输入是不是终端。几个词用一个空格连起来；要读标准
/// 输入的照原样接上，不去掉末尾的换行。凑出来是空的、只有空白的，当没有消息。
pub(crate) fn message(
    words: &[String],
    mut stdin: impl Read,
    terminal: bool,
) -> Result<String, Unread> {
    let dash = words.iter().any(|word| word == "-");
    let said = words
        .iter()
        .filter(|word| *word != "-")
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" ");
    let piped = match (words.is_empty(), dash) {
        // 终端里没写消息：照参数写错报，不等人敲。
        (true, _) if terminal => return Err(Unread::Missing),
        (true, _) | (false, true) => Some(read(&mut stdin)?),
        (false, false) => None,
    };
    let text = match piped {
        None => said,
        Some(piped) if said.is_empty() => piped,
        Some(piped) if piped.trim().is_empty() => said,
        Some(piped) => format!("{said}\n\n{piped}"),
    };
    match text.trim().is_empty() {
        true => Err(Unread::Missing),
        false => Ok(text),
    }
}

/// 凑不出要说的话：照界面语言 `language` 在标准错误上说为什么，交回退出码：没有消息、不是 UTF-8 的照参数写错，读不了的是
/// 出错。
pub(crate) fn refuse(unread: &Unread, language: Language) -> u8 {
    let (problem, code) = match unread {
        Unread::Missing => (language.missing_message(), exit::MISUSE),
        Unread::NotUtf8 => (language.stdin_not_utf8(), exit::MISUSE),
        Unread::Failed(reason) => (language.stdin_unreadable(reason), exit::ERROR),
    };
    diagnostic::error(&mut io::stderr(), language, &problem);
    code
}

/// 读到头，要是 UTF-8。
fn read(stdin: &mut impl Read) -> Result<String, Unread> {
    let mut bytes = Vec::new();
    stdin
        .read_to_end(&mut bytes)
        .map_err(|error| Unread::Failed(error.to_string()))?;
    String::from_utf8(bytes).map_err(|_| Unread::NotUtf8)
}

#[cfg(test)]
mod tests;
