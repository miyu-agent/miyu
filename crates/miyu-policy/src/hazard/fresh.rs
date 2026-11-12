//! 同一条命令里 `mktemp` 刚建的临时路径（施工 HZ-2，2026-11-12 项目主人定）：`x=$(mktemp …)` 以后不带后缀地删 `$x`
//! （`rm -rf $x`、`"$x"`、`${x}`、`"${x}"`）不问。`mktemp` 每次新建一个唯一的空路径，删的就是它自己刚建的；写测试、脚本最常见
//! 的就是这么写，每次都问只会让人习惯性点允许。
//!
//! 放宽的是权限，拿不准的一律照旧判：
//! - 只认不带后缀的引用：`mktemp` 失败时 `x` 是空的，`rm -rf $x/*` 就是 `rm -rf /*`；空的 `$x` 什么也删不到。
//! - 赋值正好是一个命令替换、里面只有一条参数全写死的 `mktemp`（[`mktemp`]）；前后拼了字的、两条命令的、参数里有变量的不算。
//! - 赋值要一定在这个 shell 里跑到：单独成句，不在括号、花括号、`if`、循环里，前面不是 `&&`、`||`，后面不是 `&`；不然
//!   `x` 可能还是外面带进来的值。
//! - 赋值以后、删以前改过 `x` 的不算（[`may_change`]）；改动不照先后跑的（循环里、函数里、`trap`）整层不认，`eval`、
//!   `source`、`.`、`case`、`IFS` 出现的整层不认。
//! - 只认同一条命令里的：替换里、`sh -c` 里的另起一份，上一次调用里赋的不算。

use std::collections::BTreeSet;

use super::lex::{Parsed, Simple, Word, parse};
use super::rules::{base, unwrap};
use super::vars::{name_of, valid};

/// 这一层（一条命令，或者替换、`sh -c` 里的一段）里记下的刚建的临时路径。
pub(super) struct Fresh {
    /// 这时 `x` 是 `mktemp` 刚建的那些名字。
    names: BTreeSet<String>,
    /// 和管道一一对上：一定在这个 shell 里跑到、不在 `if`、循环里。
    plain: Vec<bool>,
    /// 整层都不认的名字。
    banned: BTreeSet<String>,
    /// 整层什么名字都不认。
    banned_all: bool,
}

/// 开一段块的关键字：之后到配上的那一个为止都在块里。
const OPENERS: [&str; 6] = ["if", "while", "until", "for", "select", "case"];
/// 收一段块的关键字。
const CLOSERS: [&str; 3] = ["fi", "done", "esac"];
/// 块中间的关键字：带着它的那一句在块里。
const MIDDLES: [&str; 6] = ["then", "elif", "else", "do", "!", "function"];
/// 能照名字改变量的内建命令：后面的词里出现这个名字（不是 `$名字`）就算改过。
const SETTERS: [&str; 15] = [
    "read",
    "mapfile",
    "readarray",
    "unset",
    "getopts",
    "printf",
    "let",
    "declare",
    "typeset",
    "local",
    "export",
    "readonly",
    "wait",
    "for",
    "select",
];
/// 在这个 shell 里跑一段看不到的代码：出现了整层不认。
const OPAQUE: [&str; 5] = ["eval", "source", ".", "trap", "alias"];

impl Fresh {
    /// 照这一层的原文 `text` 和拆出来的 `parsed` 看：每条管道在不在块里，哪些名字整层不认。
    pub(super) fn new(text: &str, parsed: &Parsed) -> Fresh {
        let mut banned_all = contains_token(text, "IFS");
        let mut plain = Vec::with_capacity(parsed.pipelines.len());
        let mut depth = 0usize;
        for (pipeline, &lexed) in parsed.pipelines.iter().zip(&parsed.plain) {
            let mut block = depth > 0 || pipeline.len() > 1;
            for simple in pipeline {
                for word in simple.words.iter().take_while(|word| keyword(&word.text)) {
                    let word = word.text.as_str();
                    block = true;
                    if word == "case" {
                        banned_all = true;
                    }
                    if OPENERS.contains(&word) {
                        depth += 1;
                    } else if CLOSERS.contains(&word) {
                        depth = depth.saturating_sub(1);
                    }
                    // `for x`、`case $a`：后面的是名字、词，不是关键字。
                    if matches!(word, "for" | "select" | "case") {
                        break;
                    }
                }
                if unwrap(&simple.words)
                    .first()
                    .is_some_and(|word| OPAQUE.contains(&base(&word.text)))
                {
                    banned_all = true;
                }
            }
            plain.push(lexed && !block);
        }
        let banned = parsed
            .pipelines
            .iter()
            .flatten()
            .flat_map(|simple| &simple.words)
            .filter_map(|word| mktemp_name(word).or_else(|| name_of(&word.text).map(|(n, _)| n)))
            .filter(|name| {
                modified_in_text(text, name)
                    || parsed
                        .pipelines
                        .iter()
                        .zip(&plain)
                        .any(|(pipeline, &plain)| {
                            !plain && pipeline.iter().any(|simple| may_change(simple, name))
                        })
            })
            .map(str::to_string)
            .collect();
        Fresh {
            names: BTreeSet::new(),
            plain,
            banned,
            banned_all,
        }
    }

    /// 第 `at` 条管道跑过以后：改过的名字忘掉；单独成句、一定跑到的 `x=$(mktemp …)` 记下。
    pub(super) fn step(&mut self, at: usize, pipeline: &[Simple]) {
        self.names
            .retain(|name| !pipeline.iter().any(|simple| may_change(simple, name)));
        let [simple] = pipeline else {
            return;
        };
        if self.banned_all || !self.plain.get(at).copied().unwrap_or(false) {
            return;
        }
        let mut words = simple.words.as_slice();
        if words.first().is_some_and(|word| {
            matches!(
                word.text.as_str(),
                "export" | "local" | "declare" | "typeset" | "readonly"
            )
        }) {
            words = &words[1..];
            // 带选项的（`declare -i`、`-n`）值不是原样：不认。
            if words.iter().any(|word| word.text.starts_with('-')) {
                return;
            }
        }
        if words.is_empty() || !words.iter().all(|word| name_of(&word.text).is_some()) {
            return;
        }
        for word in words {
            match mktemp_name(word) {
                Some(name) if !self.banned.contains(name) => {
                    self.names.insert(name.to_string());
                }
                _ => {
                    if let Some((name, _)) = name_of(&word.text) {
                        self.names.remove(name);
                    }
                }
            }
        }
    }

    /// `rm` 的这个目标就是刚建的临时路径：去掉引号以后正好是 `$x` 或者 `${x}`，没有别的字、没有命令替换。
    pub(super) fn covers(&self, word: &Word) -> bool {
        if !word.unresolved || !word.substitutions.is_empty() {
            return false;
        }
        let Some(rest) = word.text.strip_prefix('$') else {
            return false;
        };
        let name = rest
            .strip_prefix('{')
            .and_then(|inner| inner.strip_suffix('}'))
            .unwrap_or(rest);
        valid(name) && self.names.contains(name)
    }
}

/// `x=$(mktemp …)`、`` x=`mktemp …` ``：交回名字。值得正好是一个命令替换，里面是 [`mktemp`]。
fn mktemp_name(word: &Word) -> Option<&str> {
    let [substitution] = word.substitutions.as_slice() else {
        return None;
    };
    let name = word.text.strip_suffix("=$")?;
    (word.unresolved && substitution.command && valid(name) && mktemp(&substitution.inner))
        .then_some(name)
}

/// 替换里正好一条 `mktemp`：没有别的命令、管道、重定向、放到后台、套着的替换；参数都写死（没有变量、通配、空白），只认
/// `-d`、`-u`、`-q`、`-t`（可以连写）、`-p 目录`、`--directory`、`--dry-run`、`--quiet`、`--tmpdir[=目录]`，加至多一个模板。
fn mktemp(inner: &str) -> bool {
    let parsed = parse(inner);
    let ([pipeline], [true]) = (parsed.pipelines.as_slice(), parsed.plain.as_slice()) else {
        return false;
    };
    let [simple] = pipeline.as_slice() else {
        return false;
    };
    let Some((first, rest)) = simple.words.split_first() else {
        return false;
    };
    if !parsed.inner.is_empty() || !simple.writes.is_empty() || first.text != "mktemp" {
        return false;
    }
    let literal = |word: &Word| {
        !word.unresolved
            && !word
                .text
                .contains(|c: char| c.is_whitespace() || matches!(c, '*' | '?' | '[' | '{'))
    };
    let mut templates = 0;
    let mut words = rest.iter();
    while let Some(word) = words.next() {
        if !literal(word) {
            return false;
        }
        let text = word.text.as_str();
        let short = |text: &str| {
            text.strip_prefix('-').is_some_and(|flags| {
                !flags.is_empty() && flags.chars().all(|c| matches!(c, 'd' | 'u' | 'q' | 't'))
            })
        };
        match text {
            "-p" => {
                if !words.next().is_some_and(literal) {
                    return false;
                }
            }
            "--directory" | "--dry-run" | "--quiet" | "--tmpdir" => {}
            _ if text.starts_with("--tmpdir=") || short(text) => {}
            _ if text.starts_with('-') => return false,
            _ => templates += 1,
        }
    }
    templates <= 1
}

/// 这一个简单命令可能改 `name`：有词以 `name=`、`name+=`、`name[` 开头（赋值、`export`、`let`、只给一个命令的也算），
/// 或者它是 [`SETTERS`] 里的、后面的词里出现这个名字（`read x`、`printf -v x`、`for x`、`declare -n r=x`）。
fn may_change(simple: &Simple, name: &str) -> bool {
    let assigns = simple.words.iter().chain(&simple.writes).any(|word| {
        word.text
            .strip_prefix(name)
            .is_some_and(|rest| rest.starts_with(['=', '[']) || rest.starts_with("+="))
    });
    assigns
        || unwrap(&simple.words).first().is_some_and(|first| {
            SETTERS.contains(&base(&first.text))
                && simple.words.iter().any(|word| named(&word.text, name))
        })
}

/// 原文里有不照先后、看不全的改法：`${name:=…}`、`${name=…}` 这类带修饰的（heredoc 正文里的也算），`((…))`、`$((…))`
/// 里出现这个名字。
fn modified_in_text(text: &str, name: &str) -> bool {
    let braced = format!("${{{name}");
    let modifier = text.match_indices(&braced).any(|(at, _)| {
        text[at + braced.len()..]
            .chars()
            .next()
            .is_some_and(|c| c != '}' && !ident(c))
    });
    modifier
        || text.match_indices("((").any(|(at, _)| {
            let span = &text[at + 2..];
            let span = &span[..span.find("))").unwrap_or(span.len())];
            contains_token(span, name)
        })
}

/// 词里出现 `name` 这个名字，不是 `$name`、`${name`。
fn named(text: &str, name: &str) -> bool {
    tokens(text, name).any(|at| {
        let before = &text[..at];
        !(before.ends_with('$') || before.ends_with("${"))
    })
}

/// 文字里出现 `name` 这个名字（前后不连着字母、数字、下划线）。
fn contains_token(text: &str, name: &str) -> bool {
    tokens(text, name).next().is_some()
}

/// `name` 作为一个完整名字出现的每一处的位置。
fn tokens<'a>(text: &'a str, name: &'a str) -> impl Iterator<Item = usize> + 'a {
    text.match_indices(name)
        .map(|(at, _)| at)
        .filter(move |&at| {
            !text[..at].chars().next_back().is_some_and(ident)
                && !text[at + name.len()..].chars().next().is_some_and(ident)
        })
}

fn ident(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
}

fn keyword(text: &str) -> bool {
    OPENERS.contains(&text) || CLOSERS.contains(&text) || MIDDLES.contains(&text)
}
