//! 喂给 shell 跑的（施工 FX-22 从 `rules.rs` 拆出来）：从标准输入读的脚本（heredoc、here-string、管道前面印的字）、`env -S`、
//! `su -c` 的值，和先下载、再交给 shell 或解释器的那几种写法。

use super::lex::{Simple, Word, parse};
use super::rules::{base, is_shell, unwrap};

/// shell 没写脚本文件，从标准输入读脚本（施工 FX-22）：后面的词都是选项（`-s` 以后的是给脚本的参数，也算）。
pub(super) fn reads_stdin(rest: &[Word]) -> bool {
    rest.iter()
        .take_while(|word| word.text != "-s" && word.text != "--")
        .all(|word| word.text.starts_with('-') || word.text.starts_with('+'))
}

/// 外壳里 `env` 的 `-S`、`--split-string` 的值（施工 FX-22）：连写的（`-S'…'`、`--split-string=…`）也认。
pub(super) fn split_string(wrappers: &[Word]) -> Option<&str> {
    let at = wrappers.iter().position(|word| base(&word.text) == "env")?;
    let mut words = wrappers[at + 1..].iter();
    while let Some(word) = words.next() {
        let text = word.text.as_str();
        if matches!(text, "-S" | "--split-string") {
            return words.next().map(|word| word.text.as_str());
        }
        if let Some(value) = text.strip_prefix("--split-string=") {
            return Some(value);
        }
        if !text.starts_with("--")
            && let Some(value) = text.strip_prefix("-S")
        {
            return Some(value);
        }
        if !text.starts_with('-') {
            return None;
        }
    }
    None
}

/// `su` 的 `-c`、`--command` 的值（施工 FX-22）：连写的 `--command=…` 也认。
pub(super) fn su_command(rest: &[Word]) -> Option<&str> {
    let mut words = rest.iter();
    while let Some(word) = words.next() {
        let text = word.text.as_str();
        if matches!(text, "-c" | "--command") {
            return words.next().map(|word| word.text.as_str());
        }
        if let Some(value) = text.strip_prefix("--command=") {
            return Some(value);
        }
    }
    None
}

/// 一条管道里后面的 shell 从标准输入读的脚本（施工 FX-22）：前面紧挨着的是 `echo`、`printf` 印的字（`\n` 照换行），或者不带文件
/// 的 `cat` 读进来的 heredoc、here-string。别的程序印的说不清，不认。
pub(super) fn piped_scripts(pipeline: &[Simple]) -> Vec<String> {
    let mut scripts = Vec::new();
    let mut feeding: Vec<String> = Vec::new();
    for simple in pipeline {
        let words = unwrap(&simple.words);
        let Some((first, rest)) = words.split_first() else {
            feeding.clear();
            continue;
        };
        let name = base(&first.text);
        if is_shell(name) && reads_stdin(rest) {
            scripts.append(&mut feeding);
        }
        let printed = |rest: &[Word]| {
            rest.iter()
                .map(|word| word.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
                .replace("\\n", "\n")
        };
        feeding = match name {
            "echo" => vec![printed(
                &rest[rest
                    .iter()
                    .take_while(|word| {
                        word.text.len() > 1
                            && word.text.starts_with('-')
                            && word.text[1..].chars().all(|c| matches!(c, 'n' | 'e' | 'E'))
                    })
                    .count()..],
            )],
            "printf" => vec![printed(rest)],
            "cat" if rest.iter().all(|word| word.text.starts_with('-')) => simple.stdin.clone(),
            _ => Vec::new(),
        };
    }
    scripts
}

/// 一条管道里先下载、后面接着 shell 或者解释器（读标准输入的那种）：`curl … | sh`。前面那条是在替换里下载的也算
/// （`cat <(curl …) | bash`，施工 SH-3）。
pub(super) fn fetch_into_shell(pipeline: &[Simple]) -> bool {
    let mut fetched = false;
    for simple in pipeline {
        let words = unwrap(&simple.words);
        let Some(first) = words.first() else {
            continue;
        };
        let name = base(&first.text);
        if matches!(name, "curl" | "wget") || fetches_inside(simple) {
            fetched = true;
        } else if fetched
            && interpreter(name)
            && !words[1..].iter().any(|word| !word.text.starts_with('-'))
        {
            return true;
        }
    }
    false
}

/// `bash <(curl …)`、`sh -c "$(curl …)"`、`bash <<< "$(curl …)"`：这一条的替换里在下载，它自己是 shell、解释器；在这个 shell
/// 里执行的 `source`、`.`、`eval` 也算（施工 AS-9）。只看装着替换的这一条（施工 SH-3）：原来看整条命令里有没有解释器，
/// `t=$(curl …); python3 a.py "$t"` 这种拿下载的字当数据的也报。
pub(super) fn runs_fetched(simple: &Simple) -> bool {
    fetches_inside(simple)
        && unwrap(&simple.words).first().is_some_and(|word| {
            let name = base(&word.text);
            interpreter(name) || matches!(name, "source" | "." | "eval")
        })
}

/// 这一条有一段替换在下载。
fn fetches_inside(simple: &Simple) -> bool {
    simple.substitutions.iter().any(|inner| fetches(inner))
}

fn fetches(text: &str) -> bool {
    parse(text).pipelines.iter().flatten().any(|simple| {
        unwrap(&simple.words)
            .first()
            .is_some_and(|word| matches!(base(&word.text), "curl" | "wget"))
    })
}

pub(super) fn interpreter(name: &str) -> bool {
    is_shell(name) || matches!(name, "python" | "python3" | "perl" | "ruby" | "node")
}
