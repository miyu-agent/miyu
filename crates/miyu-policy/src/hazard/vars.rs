//! 同一条命令里赋过值的变量（施工 5-17 再补，2026-10-22 项目主人定）：`T=/tmp/x; rm -rf $T` 照删 `/tmp/x` 判，不再算「要到运行时
//! 才知道」。只认单独成句的赋值（`T=…`、`export T=…` 这类），照出现的先后记；值里又引了别的变量的，照记下的换，换不掉的忘掉
//! 这个名字（之后照说不清算）。`~` 开头的值照家目录接（shell 赋值时就换了）。替换里、`sh -c` 里的另起一份。

use std::collections::BTreeMap;

use super::lex::{Simple, Word};

/// 记下的变量。`for`、`read`、`unset` 碰过的名字忘掉。
pub(super) struct Vars<'a> {
    known: BTreeMap<String, String>,
    /// 系统的家目录：没赋过值的 `$HOME` 照它换。
    home: Option<&'a str>,
}

impl<'a> Vars<'a> {
    pub(super) fn new(home: Option<&'a str>) -> Vars<'a> {
        Vars {
            known: BTreeMap::new(),
            home,
        }
    }

    /// 单独成句的赋值：记下来。管道里的（在子 shell 里赋）、后面跟着命令的（只给那个命令）不算。
    pub(super) fn assign(&mut self, pipeline: &[Simple]) {
        let [simple] = pipeline else {
            return;
        };
        let mut words = after_keywords(&simple.words);
        // 循环变量、`read` 读进来的：值要到运行时才知道，忘掉。
        match words.first().map(|word| word.text.as_str()) {
            Some("for" | "select") => {
                if let Some(name) = words.get(1) {
                    self.known.remove(&name.text);
                }
                return;
            }
            Some("read" | "mapfile" | "readarray" | "unset") => {
                for word in &words[1..] {
                    self.known.remove(&word.text);
                }
                return;
            }
            _ => {}
        }
        if words.first().is_some_and(|word| {
            matches!(
                word.text.as_str(),
                "export" | "local" | "declare" | "typeset" | "readonly"
            )
        }) {
            words = &words[1..];
            while words.first().is_some_and(|word| word.text.starts_with('-')) {
                words = &words[1..];
            }
        } else if !words.iter().all(|word| name_of(&word.text).is_some()) {
            return;
        }
        for word in words {
            let Some((name, value)) = name_of(&word.text) else {
                continue;
            };
            let value = match self.home {
                Some(home) if value == "~" || value.starts_with("~/") => {
                    format!("{home}{}", &value[1..])
                }
                _ => value.to_string(),
            };
            let value = self.expand(&Word {
                text: value,
                unresolved: word.unresolved,
            });
            if value.unresolved {
                self.known.remove(name);
            } else {
                self.known.insert(name.to_string(), value.text);
            }
        }
    }

    /// 把词里的 `$名字`、`${名字}` 换成记下的（`${名字:?}` 这类带修饰的照名字换）：全换得掉的交回换好的（不再说不清），有一个
    /// 换不掉的交回原样。
    pub(super) fn expand(&self, word: &Word) -> Word {
        if !word.unresolved {
            return word.clone();
        }
        let mut text = String::new();
        let mut rest = word.text.as_str();
        while let Some(at) = rest.find('$') {
            text.push_str(&rest[..at]);
            let after = &rest[at + 1..];
            let (name, next) = match after.strip_prefix('{') {
                Some(inner) => match inner.find('}') {
                    Some(end) => (&inner[..end], &inner[end + 1..]),
                    None => return word.clone(),
                },
                None => {
                    let end = after
                        .find(|c: char| c != '_' && !c.is_ascii_alphanumeric())
                        .unwrap_or(after.len());
                    (&after[..end], &after[end..])
                }
            };
            // `${HOME:?}`、`${T:-默认}`、`${T=…}` 有值时就是它的值，照它换（施工 AS-9，shellcheck 推荐 `${HOME:?}`）；
            // `${T:+…}` 是另一个值、`${#T}` 是长度，换不掉。
            let name = match name.find([':', '?', '-', '=', '+']) {
                Some(at) if !name[at..].trim_start_matches(':').starts_with('+') => &name[..at],
                Some(_) => return word.clone(),
                None => name,
            };
            let value = match self.known.get(name) {
                Some(value) => value.as_str(),
                None if name == "HOME" => match self.home {
                    Some(home) => home,
                    None => return word.clone(),
                },
                None => return word.clone(),
            };
            text.push_str(value);
            rest = next;
        }
        text.push_str(rest);
        Word {
            text,
            unresolved: false,
        }
    }
}

/// 去掉开头的 `if`、`then`、`do`、`else` 这类关键字：`if …; then rm -rf ~; fi` 里的 `rm` 照样认。
pub(super) fn after_keywords(words: &[Word]) -> &[Word] {
    let skip = words
        .iter()
        .take_while(|word| {
            matches!(
                word.text.as_str(),
                "if" | "then" | "elif" | "else" | "do" | "while" | "until" | "!"
            )
        })
        .count();
    &words[skip..]
}

/// `名字=值`：交回名字和值。
fn name_of(text: &str) -> Option<(&str, &str)> {
    text.split_once('=').filter(|(name, _)| valid(name))
}

/// 变量名：字母、数字、下划线，不以数字开头。
fn valid(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        && name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
}
