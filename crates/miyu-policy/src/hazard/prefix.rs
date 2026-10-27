//! 越过沙盒的放行规则照的命令前缀（施工 SB-3，终端第 23 条，2026-10-27 项目主人定，照 Claude Code）：人选了「此目录不再询问」，
//! 这个会话在这个目录下同一前缀的命令越过沙盒不再问。前缀是规则能放开多大，取得宁可窄、取不清就不报（照旧每次问）：
//!
//! 1. 第一个词照写的原样，不去路径：`./docker`、`/tmp/x/docker` 和 `docker` 是三个前缀，放行过 `docker` 不放行本地叫 docker 的脚本。
//! 2. 开头有变量赋值的不报：`PATH=…`、`LD_PRELOAD=…` 能把同一个前缀换成别的程序。
//! 3. 包一层的不报（[`WRAPPERS`]）：放行一次 `timeout` 就等于放行一切。
//! 4. 「程序 + 子命令」那几个（[`SUBCOMMANDS`]）带上紧跟着的子命令；紧跟着的是选项（`git -C dir status`）的不报：选项后面哪个词是
//!    子命令要照每个程序的写法认，认错了规则就放大了。
//! 5. 管道、`&&`、`||`、`;`、`&`、换行连起来的，每一段的前缀都一样才报这一个。有命令替换（`$( )`、反引号、`<( )`）、子 shell、
//!    花括号的，前缀里有变量的，都算切不清，不报。

use super::lex::{Simple, parse};

/// 包一层的：后面跟着的才是真跑的程序，或者整个换了一个 shell。
const WRAPPERS: [&str; 19] = [
    "env", "sudo", "doas", "xargs", "nohup", "timeout", "time", "nice", "exec", "eval", "command",
    "builtin", "source", ".", "sh", "bash", "zsh", "fish", "dash",
];

/// 「程序 + 子命令」：前缀带上子命令。
const SUBCOMMANDS: [&str; 20] = [
    "git",
    "npm",
    "pnpm",
    "yarn",
    "bun",
    "npx",
    "cargo",
    "rustup",
    "docker",
    "podman",
    "kubectl",
    "helm",
    "go",
    "pip",
    "pip3",
    "uv",
    "poetry",
    "systemctl",
    "brew",
    "gh",
];

/// `command` 的前缀：取不清的没有。
pub fn prefix(command: &str) -> Option<String> {
    if !plain(command) {
        return None;
    }
    let parsed = parse(command);
    if !parsed.inner.is_empty() {
        return None;
    }
    let mut found: Option<String> = None;
    for simple in parsed.pipelines.iter().flatten() {
        // 只有重定向、空的那一段（`&&` 后面收的空段）不算一段命令。
        if simple.words.is_empty() {
            continue;
        }
        let one = of(simple)?;
        match &found {
            Some(seen) if *seen != one => return None,
            Some(_) => {}
            None => found = Some(one),
        }
    }
    found
}

/// 一段命令的前缀。
fn of(simple: &Simple) -> Option<String> {
    let (first, rest) = simple.words.split_first()?;
    if first.unresolved || first.text.is_empty() || assignment(&first.text) {
        return None;
    }
    let name = first.text.as_str();
    if WRAPPERS.contains(&name) {
        return None;
    }
    if !SUBCOMMANDS.contains(&name) {
        return Some(name.to_string());
    }
    match rest.first() {
        None => Some(name.to_string()),
        Some(next) if next.unresolved || next.text.is_empty() || next.text.starts_with('-') => None,
        Some(next) => Some(format!("{name} {}", next.text)),
    }
}

/// 开头的 `NAME=value`：名字是字母、数字、下划线，不以数字开头。
fn assignment(text: &str) -> bool {
    text.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty()
            && !name.starts_with(|c: char| c.is_ascii_digit())
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// 引号外没有子 shell、花括号（`${NAME}` 不算）、命令替换（双引号里的 `$( )`、反引号也算）。拆词的 [`parse`] 把括号、花括号当成分隔，看不出来，
/// 这里照原文先看一遍。
fn plain(command: &str) -> bool {
    let mut chars = command.chars().peekable();
    let (mut single, mut double) = (false, false);
    while let Some(c) = chars.next() {
        match c {
            '\\' if !single => {
                chars.next();
            }
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            _ if single => {}
            '`' => return false,
            '$' if chars.peek() == Some(&'(') => return false,
            // `${NAME}` 只是取变量，跳过整段。
            '$' if chars.peek() == Some(&'{') => {
                if !chars.by_ref().any(|c| c == '}') {
                    return false;
                }
            }
            '(' | ')' | '{' | '}' if !double => return false,
            _ => {}
        }
    }
    true
}

#[cfg(test)]
mod tests;
