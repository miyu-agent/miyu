//! 参数写错时说的话（施工 4-11，`docs/blueprint/cli/main.md`「参数写错时」，设计 47）：照 clap 报的错分几种，每种
//! `miyu: ` 打头一行、能照着改的另起 `hint:`，跟着界面语言，不带 clap 的用法和提示。主程序把它印在标准错误上，退出码 2。

use clap::error::{ContextKind, ContextValue, ErrorKind};

use miyu_store::human::clean;

use crate::diagnostic::{Level, Problem};
use crate::help::{self, Page, page_named};
use crate::language::Language;

/// clap 报的错说成几行（设计 47）：`miyu: ` 打头的一行，能照着改的写在 `hint:` 里。帮助、版本不是错，主程序自己印，不走这里。
pub fn misuse(error: &clap::Error, language: Language) -> Vec<String> {
    misuse_offering(error, language, &|_| None)
}

/// 同 [`misuse`]，不认识的命令照 `offered` 找是哪个软件包带的（施工 CL-7，[`crate::packages::offering`]）：找到了提示去装它，
/// 例如没装网页包时敲 `miyu web`。
pub fn misuse_offering(
    error: &clap::Error,
    language: Language,
    offered: &dyn Fn(&str) -> Option<String>,
) -> Vec<String> {
    problem(error, language, offered).lines(language, Level::Error)
}

fn problem(
    error: &clap::Error,
    language: Language,
    offered: &dyn Fn(&str) -> Option<String>,
) -> Problem {
    let chinese = language == Language::Chinese;
    let arg = clean(&first(error, ContextKind::InvalidArg));
    let value = clean(&first(error, ContextKind::InvalidValue));
    match error.kind() {
        ErrorKind::InvalidSubcommand => {
            let name = clean(&first(error, ContextKind::InvalidSubcommand));
            match (nested(error), chinese) {
                (Some(command), true) => Problem::new(format!("'{command}' 没有子命令 '{name}'"))
                    .hint(format!("运行 '{command} --help' 查看可用命令")),
                (Some(command), false) => {
                    Problem::new(format!("unknown command '{name}' for '{command}'"))
                        .hint(format!("see '{command} --help'"))
                }
                (None, _) => match offered(&name) {
                    Some(package) => language.command_not_installed(&name, &package),
                    None => language.no_such_command(&name),
                },
            }
        }
        // 嵌着子命令的那一层（`miyu sandbox`，施工 5-8）后面没写：clap 报的是那一层的全名和能写的几个。
        ErrorKind::MissingSubcommand => {
            let command = clean(&first(error, ContextKind::InvalidSubcommand));
            let valid = all(error, ContextKind::ValidSubcommand);
            match chinese {
                true => Problem::new(format!("'{command}' 需要子命令"))
                    .hint(format!("可用的有：{}", listed(&valid, "、", " 或 "))),
                false => Problem::new(format!("'{command}' requires a subcommand"))
                    .hint(format!("one of: {}", listed(&valid, ", ", " or "))),
            }
        }
        // 成对的选项少了一个（`--owner-home`、`--owner-sid`，只给提升过的自己用）：照它的名字说。
        ErrorKind::MissingRequiredArgument if arg.starts_with('-') => match chinese {
            true => Problem::new(format!("缺少选项 '{}'", option(&arg))),
            false => Problem::new(format!("missing option '{}'", option(&arg))),
        },
        // 嵌着的子命令少了必写的（施工 K-7 修，`miyu kb read notes`）：照这个命令的帮助页说它怎么写，同 `ask` 那一句。帮助页上
        // 找不到的说到它是哪个命令为止。
        ErrorKind::MissingRequiredArgument
            if nested(error).is_some_and(|path| path.split(' ').count() > 2) =>
        {
            let path = nested(error).unwrap_or_default();
            let words: Vec<&str> = path.split(' ').collect();
            let written = page_named(words[1])
                .and_then(|page| synopsis(language, page, words[2]))
                .map_or(path.clone(), |synopsis| {
                    format!("miyu {} {synopsis}", words[1])
                });
            match chinese {
                true => Problem::new("缺少参数").hint(format!("用法：{written}")),
                false => Problem::new("missing arguments").hint(format!("usage: {written}")),
            }
        }
        // 最外面那一层没有必写的了（施工 AP-2 起 `ask` 的要说的话可以从标准输入读，没有时由 `ask` 自己照同一句说）：测试查着，
        // 多了一个，这里要跟着改。
        ErrorKind::MissingRequiredArgument => language.missing_message(),
        ErrorKind::UnknownArgument => match (chinese, arg.starts_with('-')) {
            (true, true) => Problem::new(format!("无法识别的选项 '{arg}'")),
            (true, false) => Problem::new(format!("多余的参数 '{arg}'")),
            (false, true) => Problem::new(format!("unrecognized option '{arg}'")),
            (false, false) => Problem::new(format!("unexpected argument '{arg}'")),
        },
        ErrorKind::ArgumentConflict => {
            let prior = clean(&first(error, ContextKind::PriorArg));
            let (one, other) = (option(&arg), option(&prior));
            match chinese {
                true => Problem::new(format!("'{one}' 不能和 '{other}' 同时使用")),
                false => Problem::new(format!("'{one}' cannot be used with '{other}'")),
            }
        }
        ErrorKind::InvalidValue if value.is_empty() => match chinese {
            true => Problem::new(format!("选项 '{}' 需要一个值", option(&arg))),
            false => Problem::new(format!("option '{}' requires a value", option(&arg))),
        },
        // `--add-dir` 后面不是一个已经有的目录（施工 5-10 上）。
        ErrorKind::ValueValidation if option(&arg) == "--add-dir" => match chinese {
            true => Problem::new(format!("'--add-dir' 的值无效：'{value}' 不是已有的目录")),
            false => Problem::new(format!(
                "invalid value for '--add-dir': '{value}' is not an existing directory"
            )),
        },
        // `--timeout` 后面不是一个时长（施工 7-9）。
        ErrorKind::ValueValidation if option(&arg) == "--timeout" => match chinese {
            true => Problem::new(format!("'--timeout' 的值无效：'{value}'"))
                .hint("写成 30s、10m、1h 这样的时长"),
            false => Problem::new(format!("invalid value for '--timeout': '{value}'"))
                .hint("use a duration such as 30s, 10m or 1h"),
        },
        // `--from` 后面是空的、只有空白（施工 7-10）。
        ErrorKind::ValueValidation if option(&arg) == "--from" => match chinese {
            true => Problem::new("选项 '--from' 需要另一个 harness 的名字"),
            false => Problem::new("option '--from' requires the name of the other harness"),
        },
        ErrorKind::InvalidValue if !all(error, ContextKind::ValidValue).is_empty() => {
            let valid = all(error, ContextKind::ValidValue);
            match chinese {
                true => Problem::new(format!("'{}' 的值无效：'{value}'", option(&arg)))
                    .hint(format!("可选的值：{}", listed(&valid, "、", " 或 "))),
                false => Problem::new(format!("invalid value '{value}' for '{}'", option(&arg)))
                    .hint(format!("possible values: {}", listed(&valid, ", ", " or "))),
            }
        }
        _ => {
            let said = clean(clap_said(error).trim_start_matches("error: "));
            match chinese {
                true => Problem::new(format!("参数错误：{said}")),
                false => Problem::new(said),
            }
        }
    }
}

/// 这种语言的帮助页 `page` 上子命令 `sub` 那一行的写法（`read <库> <文档>`）：「命令」那一节一行一个，空两格起头，写法和
/// 说明之间至少空两格（施工 K-7 修）。没有那一行的没有。
pub(crate) fn synopsis(language: Language, page: Page, sub: &str) -> Option<String> {
    help::page(language, page)
        .lines()
        .filter_map(|line| line.strip_prefix("  "))
        .find(|line| line.split(' ').next() == Some(sub))
        .map(|line| {
            line.split("  ")
                .next()
                .unwrap_or(line)
                .trim_end()
                .to_string()
        })
}

/// 写错的子命令是不是嵌着的那一层的：看 clap 报的用法那一行（`Usage: miyu sandbox <COMMAND>`），第一个 `<`、`[`
/// 之前不止一个词的是。交回那几个词；最外面那一层（只有 `miyu`）的是空的。
fn nested(error: &clap::Error) -> Option<String> {
    let usage = match error.get(ContextKind::Usage) {
        Some(ContextValue::StyledStr(usage)) => usage.to_string(),
        _ => return None,
    };
    let words: Vec<&str> = usage
        .lines()
        .next()?
        .split_whitespace()
        .skip(1)
        .take_while(|word| !word.starts_with('<') && !word.starts_with('['))
        .collect();
    (words.len() > 1).then(|| clean(&words.join(" ")))
}

/// 报错里这一样的第一个字：一个的就是它，几个的取第一个，没有的是空的。
fn first(error: &clap::Error, kind: ContextKind) -> String {
    all(error, kind).into_iter().next().unwrap_or_default()
}

/// 报错里这一样的全部字。
fn all(error: &clap::Error, kind: ContextKind) -> Vec<String> {
    match error.get(kind) {
        Some(ContextValue::String(one)) => vec![one.clone()],
        Some(ContextValue::Strings(many)) => many.clone(),
        _ => Vec::new(),
    }
}

/// 选项去掉后面的值名：`--session <SESSION>` 写成 `--session`。
fn option(arg: &str) -> &str {
    arg.split_whitespace().next().unwrap_or(arg)
}

/// 几个值连起来：两个的中间用 `last`，三个以上的前面用 `between`，最后一个前面用 `last`。
fn listed(values: &[String], between: &str, last: &str) -> String {
    match values {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., end] => format!("{}{last}{end}", init.join(between)),
    }
}

/// clap 的原话：它报错的第一行。
fn clap_said(error: &clap::Error) -> String {
    error
        .to_string()
        .lines()
        .next()
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests;
