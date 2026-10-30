//! `miyu ask` 凑不出要说的话时说的（施工 AP-2，设计 50 第二节，设计 47）：没有消息、标准输入不是 UTF-8、读不了标准输入。

use super::Language;
use crate::diagnostic::Problem;

impl Language {
    /// 没有消息：没写、终端里没写，标准输入读到的是空的、只有空白的。和 clap 少了必写的参数时说的是同一句（施工 4-11）。
    pub(crate) fn missing_message(&self) -> Problem {
        match self {
            Language::Chinese => Problem::new("缺少消息").hint("用法：miyu ask \"<消息>\""),
            Language::English => {
                Problem::new("missing message").hint("usage: miyu ask \"<message>\"")
            }
        }
    }

    /// 标准输入读到的不是 UTF-8 文本。
    pub(crate) fn stdin_not_utf8(&self) -> Problem {
        match self {
            Language::Chinese => Problem::new("标准输入不是 UTF-8 文本"),
            Language::English => Problem::new("standard input is not UTF-8 text"),
        }
    }

    /// 读标准输入出了错：`reason` 是底层的原话。
    pub(crate) fn stdin_unreadable(&self, reason: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("无法读取标准输入：{reason}")),
            Language::English => Problem::new(format!("cannot read standard input: {reason}")),
        }
    }
}
