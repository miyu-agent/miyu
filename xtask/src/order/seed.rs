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
//!
//! 打包时探针跑完一轮以后（施工 MO-4，总施工单第 3 条），[`merge`] 把这一轮的结果合进起点清单，写出给下一次用的名字清单；
//! `cargo xtask order-seed` 收回仓库之前用 [`problems`] 查格式。

use std::collections::{BTreeSet, HashMap, HashSet};

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

/// 探针这一轮碰过的代码页（量尺写的 `<清单>.pages`，格式见 `crates/miyu-perf/src/order/pages.rs` 的 `touched_text`）。
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Touched {
    /// 页大小。
    pub(crate) page: u64,
    /// 探针关上的那一段（起点算、终点不算）：`__text` 整页的部分。段外的代码（`__text` 开头、末尾不满一页的两截）探针
    /// 看不见，落在段外的函数一律当作碰过，不去掉：`main` 和启动时执行的几个函数就排在 `__text` 开头不满一页的那一截里
    /// （2026-11-23 在一台 M1 MacBook 上，没有这一条时第一轮去掉了它们）。
    pub(crate) covered: (u64, u64),
    /// 每页的起点（符号表里的地址）。
    pub(crate) starts: BTreeSet<u64>,
}

impl Touched {
    /// 读 `<清单>.pages` 的文本。
    ///
    /// # Errors
    ///
    /// 第一行不是 `page <页大小>`、页大小是 0、某一行不是十六进制的数、一页都没有。
    pub(crate) fn parse(text: &str) -> Result<Touched, String> {
        let mut lines = text.lines();
        let first = lines.next().unwrap_or_default();
        let page = first
            .strip_prefix("page ")
            .ok_or("第一行要是 `page <页大小>`")?
            .parse::<u64>()
            .map_err(|_| format!("第一行 `{first}` 的页大小不是数"))?;
        if page == 0 {
            return Err("页大小是 0".to_string());
        }
        let second = lines.next().unwrap_or_default();
        let covered = match second
            .strip_prefix("covered ")
            .and_then(|range| range.split_once(' '))
        {
            Some((start, end)) => (
                u64::from_str_radix(start, 16).map_err(|_| format!("`{second}` 的起点不是数"))?,
                u64::from_str_radix(end, 16).map_err(|_| format!("`{second}` 的终点不是数"))?,
            ),
            None => return Err("第二行要是 `covered <起点> <终点>`".to_string()),
        };
        let starts = lines
            .map(|line| u64::from_str_radix(line, 16).map_err(|_| format!("`{line}` 不是页的起点")))
            .collect::<Result<BTreeSet<u64>, String>>()?;
        if starts.is_empty() {
            return Err("一页都没有".to_string());
        }
        Ok(Touched {
            page,
            covered,
            starts,
        })
    }

    /// 从 `start` 起 `size` 字节的函数有没有一页被碰过；有一截落在探针关上的那一段以外的，当作碰过。
    fn covers(&self, start: u64, size: u64) -> bool {
        let last = start + size.max(1) - 1;
        if start < self.covered.0 || last >= self.covered.1 {
            return true;
        }
        let first = start - start % self.page;
        self.starts.range(first..=last).next().is_some()
    }
}

/// 合并的结果（[`merge`]）。
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Merged {
    /// 下一次的起点清单，一个名字一项。
    pub(crate) names: Vec<String>,
    /// 这一轮新看到的、起点清单里没有的名字数。
    pub(crate) added: usize,
    /// 因为落在这一轮一页都没被碰过的页上去掉的名字数。
    pub(crate) removed: usize,
    /// 在这一份程序里对不上符号、照原样留着的名字数（没有页可看，不去）。
    pub(crate) unmatched: usize,
}

/// 把探针这一轮的结果合进起点清单（总施工单第 3 条的三条规则）：
///
/// 1. 起点清单 `seed` 里的名字照原来的先后留着；
/// 2. 这一轮看到的函数 `seen`（量尺写的清单，Mach-O 的符号，照第一次看到的先后）里起点清单没有的，换成名字接在后面；
/// 3. 起点清单里的名字，在程序的符号表 `functions`（`verify::sized` 的（符号、起点、大小））里对上的几个符号没有一个落在
///    `touched` 碰过的页上（落在探针关上的那一段以外的算碰过，见 [`Touched::covered`]）、这一轮也没看到它的，去掉：清单里的函数挤在代码段开头，一整页都没被碰过，说明这一页上的函数
///    这一次都没用到。没有这一条，清单只会越来越长，排在前面的那一段一个版本比一个版本宽。
///
/// 对不上符号的名字（这一份程序里没有它）照原样留着，只数出来：没有页可看，不算「没被碰过」。
pub(crate) fn merge(
    seed: &str,
    seen: &[&str],
    functions: &[(&str, u64, u64)],
    touched: &Touched,
) -> Merged {
    let mut places: HashMap<String, Vec<(u64, u64)>> = HashMap::new();
    for &(symbol, start, size) in functions {
        places
            .entry(name_of(symbol))
            .or_default()
            .push((start, size));
    }
    let mut seen_names = Vec::new();
    let mut seen_set = HashSet::new();
    for symbol in seen {
        let name = name_of(symbol);
        if seen_set.insert(name.clone()) {
            seen_names.push(name);
        }
    }
    let listed = super::verify::names(seed);
    let mut names = Vec::new();
    let (mut removed, mut unmatched) = (0, 0);
    for name in &listed {
        match places.get(*name) {
            None => {
                unmatched += 1;
                names.push((*name).to_string());
            }
            Some(at)
                if seen_set.contains(*name)
                    || at.iter().any(|&(start, size)| touched.covers(start, size)) =>
            {
                names.push((*name).to_string());
            }
            Some(_) => removed += 1,
        }
    }
    let had: HashSet<&str> = listed.iter().copied().collect();
    let fresh: Vec<String> = seen_names
        .into_iter()
        .filter(|name| !had.contains(name.as_str()))
        .collect();
    let added = fresh.len();
    names.extend(fresh);
    Merged {
        names,
        added,
        removed,
        unmatched,
    }
}

/// 一份名字清单的格式问题（`cargo xtask order-seed` 收回仓库之前查）：是空的、有空行、名字前后带空白、有重复的、写的是
/// Mach-O 的 Rust 符号而不是名字、最后一行没有换行。没有问题的是空的。
pub(crate) fn problems(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    if text.trim().is_empty() {
        return vec!["是空的".to_string()];
    }
    if !text.ends_with('\n') {
        found.push("最后一行没有换行".to_string());
    }
    let mut seen = HashSet::new();
    for (index, line) in text.lines().enumerate() {
        let at = index + 1;
        if line.is_empty() {
            found.push(format!("第 {at} 行是空的"));
        } else if line.trim() != line {
            found.push(format!("第 {at} 行前后带空白"));
        } else if line.starts_with("__R") || line.starts_with("__ZN") {
            found.push(format!("第 {at} 行是符号不是名字：{line}"));
        } else if !seen.insert(line) {
            found.push(format!("第 {at} 行和前面的重复：{line}"));
        }
    }
    found
}

#[cfg(test)]
mod tests;
