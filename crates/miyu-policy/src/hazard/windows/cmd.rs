//! 把一条 cmd 命令拆成一个个简单命令（施工 WN-1）：Windows 上她的命令走 PowerShell，cmd 的写法只在 `cmd /c "…"` 里出现，
//! 拆出来的照 PowerShell 那一路的规则认（`rd`、`del`、`format`、`shutdown` 在两边是同一个名字）。认得 `&`、`&&`、`||`、`|`、
//! 换行、括号，双引号（cmd 的引号里没有转义），`^` 转义，`>`、`>>`、`2>`、`2>&1`；开头的 `@` 去掉。
//!
//! `%名字%` 留在字里、记成「说不清」，判路径时认得的几个照家目录换（`%USERPROFILE%`）。

use super::super::lex::{Simple, Word};
use super::lex::Parsed;

/// 拆 `text`：cmd 没有替换，[`Parsed::inner`] 是空的。
pub(super) fn parse(text: &str) -> Parsed {
    let mut parsed = Parsed::default();
    let mut pipeline: Vec<Simple> = Vec::new();
    let mut simple = Simple::default();
    let mut word: Option<Word> = None;
    let mut redirect = false;
    let mut quoted = false;
    let chars: Vec<char> = text.chars().collect();
    let mut at = 0;
    let end_word = |word: &mut Option<Word>, simple: &mut Simple, redirect: &mut bool| {
        if let Some(mut done) = word.take() {
            if simple.words.is_empty() && !*redirect {
                done.text = done.text.trim_start_matches('@').to_string();
                if done.text.is_empty() {
                    return;
                }
            }
            match std::mem::take(redirect) {
                true => simple.writes.push(done),
                false => simple.words.push(done),
            }
        }
    };
    while let Some(&c) = chars.get(at) {
        at += 1;
        if quoted {
            match c {
                '"' => quoted = false,
                '%' => push(&mut word, c, true),
                c => push(&mut word, c, false),
            }
            continue;
        }
        let ends = match c {
            '"' => {
                quoted = true;
                word.get_or_insert_with(Word::default);
                None
            }
            '^' => {
                if let Some(&next) = chars.get(at) {
                    at += 1;
                    if next != '\n' {
                        push(&mut word, next, false);
                    }
                }
                None
            }
            ' ' | '\t' => Some(Ends::Word),
            '|' | '&' if chars.get(at) == Some(&c) => {
                at += 1;
                Some(Ends::Pipeline)
            }
            '|' => Some(Ends::Simple),
            '&' | '\r' | '\n' | '(' | ')' => Some(Ends::Pipeline),
            '>' => {
                if word
                    .as_ref()
                    .is_some_and(|word| word.text.chars().all(|c| c.is_ascii_digit()))
                {
                    word = None;
                }
                end_word(&mut word, &mut simple, &mut redirect);
                if chars.get(at) == Some(&'>') {
                    at += 1;
                }
                if chars.get(at) == Some(&'&') {
                    at += 1;
                    while chars.get(at).is_some_and(char::is_ascii_digit) {
                        at += 1;
                    }
                } else {
                    redirect = true;
                }
                None
            }
            '%' => {
                push(&mut word, c, true);
                None
            }
            c => {
                push(&mut word, c, false);
                None
            }
        };
        let Some(ends) = ends else {
            continue;
        };
        end_word(&mut word, &mut simple, &mut redirect);
        if matches!(ends, Ends::Simple | Ends::Pipeline) && !simple.words.is_empty() {
            pipeline.push(std::mem::take(&mut simple));
        }
        if matches!(ends, Ends::Pipeline) {
            redirect = false;
            simple = Simple::default();
            if !pipeline.is_empty() {
                parsed.pipelines.push(std::mem::take(&mut pipeline));
            }
        }
    }
    end_word(&mut word, &mut simple, &mut redirect);
    if !simple.words.is_empty() {
        pipeline.push(simple);
    }
    if !pipeline.is_empty() {
        parsed.pipelines.push(pipeline);
    }
    parsed
}

/// 一个字的边界。
enum Ends {
    Word,
    Simple,
    Pipeline,
}

fn push(word: &mut Option<Word>, c: char, unresolved: bool) {
    let word = word.get_or_insert_with(Word::default);
    word.text.push(c);
    word.unresolved |= unresolved;
}
