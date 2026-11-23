//! 核对排上了（施工 LW-1，`internal/blueprint/perf.md`「排函数」第 6 条）：只吃 `nm` 的输出和清单的文本，不碰文件系统。
//!
//! 清单里在符号表里找得到的函数合计 L 字节；这些函数里，起点落在「代码段最低的函数地址起、往后 1.25 L」以内的，按字节算
//! 要占九成以上。留两成五的余量是给对齐的空隙和链接器挪不动、插在中间的几段；九成以上才算数，换了链接器、参数没生效时
//! 这些函数散在整个代码段里，落在开头的只有一小部分。
//!
//! 苹果的 `nm`（llvm-nm）读 Mach-O 时 `-S` 那一列全是 0（施工 MO-1）：列出的函数大小都是 0 时，每个函数的大小改用「下一个
//! 符号（任何类型）的起点减去自己的起点」，最后一个后面没有符号的给 [`LAST`] 字节。和
//! `crates/miyu-perf/src/order/listing.rs` 的 `Symbols::parse` 是同一个算法，改一边要改另一边。Linux 的表里总有带大小的
//! 函数，不补，行为和以前一样。苹果列成 `T` 的文件头 [`HEADER`] 不算函数。名字照原样比：Mach-O 的符号开头多一个下划线，
//! 清单里写的也是符号的原名。

use std::collections::{HashMap, HashSet};
use std::fmt;

/// 苹果的符号表里，最后一个函数后面一个符号都没有时给它的大小：一页（Apple 芯片上是 16 kB）。`nm` 不列节的范围，拿不到
/// `__text` 的末尾；真实的程序里代码段后面总还有别的节的符号，用不到这一条。
const LAST: u64 = 16 * 1024;

/// Mach-O 的文件头：苹果的 `nm` 把它列成 `T`，地址是 `__TEXT` 段的起点、在第一个函数前面，它不是函数。算进来的话代码段的
/// 起点要往前挪一截（核心是 0xac0 字节）。Linux 的符号表里没有这个名字。
const HEADER: &str = "__mh_execute_header";

/// 落在开头的字节要占的比例：分子、分母。
const SHARE: (u64, u64) = (9, 10);

/// 核对的结果：清单多少个名字、找到多少个、合计多少字节、其中多少落在开头。
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Placed {
    /// 清单里不重复的名字数。
    pub(crate) listed: usize,
    /// 其中在符号表里找得到的。
    pub(crate) found: usize,
    /// 找得到的函数合计多少字节（L）。
    pub(crate) bytes: u64,
    /// 其中起点落在开头 1.25 L 以内的字节数。
    pub(crate) near: u64,
}

impl fmt::Display for Placed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "清单 {} 个函数，符号表里找到 {} 个、合计 {} 字节，{}% 排在代码段开头",
            self.listed,
            self.found,
            self.bytes,
            percent(self.near, self.bytes)
        )
    }
}

/// 清单里的名字：一行一个，去掉首尾空白，空行不算，重复的只留第一次。
pub(crate) fn names(list: &str) -> Vec<&str> {
    let mut seen = HashSet::new();
    list.lines()
        .map(str::trim)
        .filter(|name| !name.is_empty() && seen.insert(*name))
        .collect()
}

/// 照 `nm --defined-only --numeric-sort -S` 的输出核对清单排在了代码段开头。
///
/// 只看代码段里的函数（类型 `t`、`T`、`w`、`W`）。同名的局部函数可以有好几个，链接器照名字排的时候一起挪，这里也一起算。
///
/// # Errors
///
/// 清单是空的、清单里的名字在符号表里一个都找不到、落在开头的不到九成：交回比例和可能的原因。
pub(crate) fn check(nm: &str, list: &str) -> Result<Placed, String> {
    let listed = names(list);
    if listed.is_empty() {
        return Err("清单是空的".to_string());
    }
    let functions = functions(nm);
    let Some(start) = functions.iter().map(|function| function.address).min() else {
        return Err("nm 没列出代码段里的函数".to_string());
    };
    let mut by_name: HashMap<&str, Vec<&Function>> = HashMap::new();
    for function in &functions {
        by_name.entry(function.name).or_default().push(function);
    }
    let wanted: Vec<&Function> = listed
        .iter()
        .filter_map(|name| by_name.get(name))
        .flatten()
        .copied()
        .collect();
    let found = listed
        .iter()
        .filter(|name| by_name.contains_key(*name))
        .count();
    let bytes: u64 = wanted.iter().map(|function| function.size).sum();
    if bytes == 0 {
        return Err(format!(
            "清单里的 {} 个名字在符号表里一个都找不到（或者都没有大小）：清单和这个程序对不上",
            listed.len()
        ));
    }
    let end = start.saturating_add(bytes + bytes / 4);
    let near = wanted
        .iter()
        .filter(|function| function.address < end)
        .map(|function| function.size)
        .sum();
    let placed = Placed {
        listed: listed.len(),
        found,
        bytes,
        near,
    };
    if near * SHARE.1 < bytes * SHARE.0 {
        return Err(format!(
            "{placed}，不到九成：可能换了链接器，或者链接参数没生效（排序清单在 Linux 上是 lld 的参数，在 macOS 上是 ld64 的 \
             -order_file）"
        ));
    }
    Ok(placed)
}

/// 代码段里的一个函数。
#[derive(Debug)]
struct Function<'a> {
    address: u64,
    size: u64,
    name: &'a str,
}

/// 读 `nm -S` 的输出：`地址 大小 类型 名字`；没有大小的那种（`地址 类型 名字`）大小记作 0，地址仍然算进代码段的起点。
/// 列出的函数大小全是 0 的（苹果的 `nm`），大小照下一个符号的起点补（见模块的说明）。
fn functions(nm: &str) -> Vec<Function<'_>> {
    let rows: Vec<(u64, u64, &str, &str)> = nm
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let (address, size, kind, name) = match fields.as_slice() {
                [address, size, kind, name] => (*address, Some(*size), *kind, *name),
                [address, kind, name] => (*address, None, *kind, *name),
                _ => return None,
            };
            let address = u64::from_str_radix(address, 16).ok()?;
            let size = match size {
                Some(size) => u64::from_str_radix(size, 16).ok()?,
                None => 0,
            };
            Some((address, size, kind, name))
        })
        .collect();
    let mut functions: Vec<Function<'_>> = rows
        .iter()
        .filter(|&&(_, _, kind, name)| matches!(kind, "t" | "T" | "w" | "W") && name != HEADER)
        .map(|&(address, size, _, name)| Function {
            address,
            size,
            name,
        })
        .collect();
    if !functions.is_empty() && functions.iter().all(|function| function.size == 0) {
        let mut starts: Vec<u64> = rows.iter().map(|&(address, ..)| address).collect();
        starts.sort_unstable();
        starts.dedup();
        for function in &mut functions {
            let next = starts.partition_point(|&start| start <= function.address);
            function.size = starts
                .get(next)
                .map_or(LAST, |&start| start - function.address);
        }
    }
    functions
}

/// 整数的百分比，向下取整。
fn percent(part: u64, whole: u64) -> u64 {
    part.saturating_mul(100).checked_div(whole).unwrap_or(0)
}

#[cfg(test)]
mod tests;
