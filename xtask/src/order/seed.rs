//! macOS 排函数的名字清单（施工 MO-3，总施工单 `internal/construction/MO-macOS的发行包排函数.md` 第 3、4 条）：起点清单里
//! 写的是函数名，链接器的 `-order_file` 要的是这一次的符号，这里做两者的互换。都是纯函数，不碰文件系统，Linux 上也测。
//!
//! 起点清单（`xtask/order/macos-miyu.txt`、`macos-miyu-tui.txt`）一行一个名字。Rust 的函数写 `rustc-demangle` 不带哈希的
//! 写法（`{:#}`）：crate 的哈希跟着版本号、工具链变，名字不变；C 的函数写源码里的名字。名字后面编译器加的 `.llvm.<数字>`、
//! `.<数字>` 后缀去掉，同一个函数的几份拷贝对得上同一个名字。
//!
//! 一个 Mach-O 符号换成名字（[`name_of`]）：先去掉开头的一个下划线（Mach-O 给每个 C 的名字前面加了一个），再去掉后缀；
//! 剩下的以 `_R`（v0）或 `_ZN`（legacy）开头的是 Rust 的，交给 `rustc-demangle` 解，解不开的和别的（C 函数）原样就是
//! 名字。前置任务在一台 M1 MacBook 上把这种写法和 `c++filt` 逐个比过，完全一样（2026-11-23）。

use std::collections::{HashMap, HashSet};

/// 一个 Mach-O 符号对应的起点清单里的名字（见模块的说明）。
pub(crate) fn name_of(symbol: &str) -> String {
    let bare = symbol.strip_prefix('_').unwrap_or(symbol);
    let bare = without_suffix(bare);
    let rust = bare.starts_with("_R") || bare.starts_with("_ZN");
    match rust.then(|| rustc_demangle::try_demangle(bare)) {
        Some(Ok(demangled)) => format!("{demangled:#}"),
        _ => bare.to_string(),
    }
}

/// 去掉名字末尾编译器加的后缀：`.<数字>`，以及 `.llvm.<数字>`，可以叠好几层（`.llvm.123.4`）。数字以外的后缀（例如
/// `.cold`）不去：那是另一段代码，名字和原函数不一样。
fn without_suffix(mut name: &str) -> &str {
    while let Some((head, tail)) = name.rsplit_once('.') {
        if tail.is_empty() || !tail.bytes().all(|byte| byte.is_ascii_digit()) {
            break;
        }
        name = head.strip_suffix(".llvm").unwrap_or(head);
    }
    name
}

/// 苹果的 `nm --defined-only` 输出里代码段的符号（类型 `t`、`T`），照出现的先后。读归档时 `nm` 在每个成员前面印一行
/// `<归档>(<成员>):`，读几个文件时在每个文件前面印一行 `<文件>:`；这些行、空行、别的段的符号都跳过。
pub(crate) fn code_symbols(nm: &str) -> Vec<&str> {
    nm.lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            match fields.as_slice() {
                [_, "t" | "T", symbol] => Some(*symbol),
                _ => None,
            }
        })
        .collect()
}

/// 名字换成这一次的符号的结果。
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Mapped<'a> {
    /// 起点清单里不重复的名字数。
    pub(crate) listed: usize,
    /// 其中在这一次的符号里对上的。
    pub(crate) found: usize,
    /// 写进 `-order_file` 的符号，照清单里名字的先后；一个名字对上几个符号的，照它们在符号表里出现的先后挨着放。
    pub(crate) symbols: Vec<&'a str>,
}

/// 把起点清单 `names`（一行一个名字，读法同 `verify::names`：去掉首尾空白、跳过空行、重复的只留第一次）换成 `symbols`
/// 里的符号。对不上的名字跳过，只算进 `listed`、不算进 `found`：清单旧了、函数改了名都会这样，不算错。同一个符号在几个
/// 输入里都列了的（例如几个归档里同名的局部函数）只写一次。
pub(crate) fn map<'a>(names: &str, symbols: &[&'a str]) -> Mapped<'a> {
    let mut by_name: HashMap<String, Vec<&'a str>> = HashMap::new();
    let mut seen = HashSet::new();
    for symbol in symbols.iter().copied() {
        if seen.insert(symbol) {
            by_name.entry(name_of(symbol)).or_default().push(symbol);
        }
    }
    let listed = super::verify::names(names);
    let mut found = 0;
    let mut ordered = Vec::new();
    for name in &listed {
        if let Some(matched) = by_name.get(*name) {
            found += 1;
            ordered.extend(matched.iter().copied());
        }
    }
    Mapped {
        listed: listed.len(),
        found,
        symbols: ordered,
    }
}

/// 一行一个写成清单的文本，最后一行也带换行；空的是空字符串。
pub(crate) fn render(lines: &[&str]) -> String {
    lines.iter().map(|line| format!("{line}\n")).collect()
}

#[cfg(test)]
mod tests;
