//! PowerShell、cmd 的删除（施工 WN-1）：`Remove-Item` 和它的别名（`rm`、`del`、`erase`、`rd`、`rmdir`、`ri`），cmd 的
//! `rd`、`del`。整个目录往下删的是 `-Recurse`（参数名可以只写开头，`-r`、`-rec`）、cmd 的 `/s`；bash 习惯的 `-rf` 也算。
//! 目标照 `-Path`、`-LiteralPath`（`-Path:值` 也认）和不带名字的参数，`-Filter`、`-Include` 这类的值不是目标；cmd 的单字母
//! 开关（`/s`、`/q`、`/f`）不是目标。写了 `-WhatIf` 的只是说会删什么，不判。
//!
//! 管道喂进来的（`Get-ChildItem … | Remove-Item`）照 bash 的 `find … -delete`：删那几个起点；前面不是列目录的、目标说不清的
//! 整个目录往下删，照 `xargs rm -rf` 问。

use super::super::Kind;
use super::super::lex::{Simple, Word};
use super::paths::{Tidy, target};
use super::{base, words};

/// 删除的程序名（已经小写、去掉 `.exe`）。
pub(super) fn deletes(name: &str) -> bool {
    matches!(
        name,
        "remove-item" | "rm" | "del" | "erase" | "rd" | "rmdir" | "ri"
    )
}

/// `-名字`、`-名字:值`：小写的名字和 `:` 后面的值。不是 `-` 打头的交回空的。
fn parameter(text: &str) -> Option<(String, Option<&str>)> {
    let rest = text.strip_prefix('-')?;
    if rest.is_empty() || rest.starts_with('-') {
        return None;
    }
    let (name, value) = match rest.split_once(':') {
        Some((name, value)) => (name, Some(value)),
        None => (rest, None),
    };
    Some((name.to_lowercase(), value))
}

/// 参数名 `name` 是 `full` 的开头，至少 `least` 个字（PowerShell 认参数名的开头）。
fn names(name: &str, full: &str, least: usize) -> bool {
    name.len() >= least && full.starts_with(name)
}

/// 值是目标的参数。
fn path_parameter(name: &str) -> bool {
    names(name, "path", 2) || names(name, "literalpath", 2) || matches!(name, "lp" | "pspath")
}

/// 带值、值不是目标的参数（`Remove-Item`、`Get-ChildItem` 的）。
fn valued_parameter(name: &str) -> bool {
    names(name, "filter", 2)
        || names(name, "include", 2)
        || names(name, "exclude", 2)
        || names(name, "credential", 2)
        || names(name, "stream", 2)
        || names(name, "depth", 2)
        || names(name, "attributes", 2)
}

/// cmd 的单字母开关：`/s`、`/q`、`/f`、`/a`、`/p`、`/?`。
fn switch(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next() == Some('/')
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '?')
        && chars.next().is_none()
}

/// 整个目录往下删。
fn recursive(rest: &[Word]) -> bool {
    rest.iter().any(|word| {
        let lower = word.text.to_lowercase();
        lower == "/s"
            || parameter(&word.text).is_some_and(|(name, _)| {
                names(&name, "recurse", 1)
                    || (name.contains('r') && name.chars().all(|c| matches!(c, 'r' | 'f')))
            })
    })
}

/// 一个命令的目标：带名字的照 [`path_parameter`]，不带名字的、不是开关的都算。
fn targets(rest: &[Word]) -> Vec<Word> {
    let mut found = Vec::new();
    let mut words = rest.iter();
    while let Some(word) = words.next() {
        if switch(&word.text) {
            continue;
        }
        let Some((name, value)) = parameter(&word.text) else {
            found.push(word.clone());
            continue;
        };
        let target = path_parameter(&name);
        match value {
            Some(value) if target => found.push(Word {
                text: value.to_string(),
                ..word.clone()
            }),
            Some(_) => {}
            None if target => found.extend(words.next().cloned()),
            None if valued_parameter(&name) => {
                words.next();
            }
            None => {}
        }
    }
    found
}

/// 删除的那一个命令：`rest` 是程序名后面的词，`fed` 是管道里排在它前面的那几个。
pub(super) fn deleting(
    rest: &[Word],
    fed: &[Simple],
    tidy: &Tidy,
    cwd: Option<&str>,
    found: &mut Vec<Kind>,
) {
    if rest
        .iter()
        .any(|word| parameter(&word.text).is_some_and(|(name, _)| names(&name, "whatif", 2)))
    {
        return;
    }
    let recursive = recursive(rest);
    let named = targets(rest);
    for word in &named {
        found.extend(target(word, tidy, cwd, recursive));
    }
    if !named.is_empty() || fed.is_empty() {
        return;
    }
    let listed: Vec<&[Word]> = fed
        .iter()
        .filter_map(|simple| {
            let (first, rest) = words(&simple.words).split_first()?;
            matches!(
                base(&first.text).as_str(),
                "get-childitem" | "gci" | "ls" | "dir" | "get-item" | "gi"
            )
            .then_some(rest)
        })
        .collect();
    if listed.is_empty() {
        if recursive {
            found.push(Kind::DeleteUnresolved);
        }
        return;
    }
    let here = Word {
        text: ".".to_string(),
        ..Word::default()
    };
    for rest in listed {
        let starts = targets(rest);
        let starts = if starts.is_empty() {
            vec![here.clone()]
        } else {
            starts
        };
        for word in &starts {
            found.extend(target(word, tidy, cwd, true));
        }
    }
}
