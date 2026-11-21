//! 核对排上了（施工 LW-1，`internal/blueprint/perf.md`「排函数」第 6 条）：只吃 `nm` 的输出和清单的文本，不碰文件系统。
//!
//! 清单里在符号表里找得到的函数合计 L 字节；这些函数里，起点落在「代码段最低的函数地址起、往后 1.25 L」以内的，按字节算
//! 要占九成以上。留两成五的余量是给对齐的空隙和链接器挪不动、插在中间的几段；九成以上才算数，换了链接器、参数没生效时
//! 这些函数散在整个代码段里，落在开头的只有一小部分。

use std::collections::{HashMap, HashSet};
use std::fmt;

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
            "{placed}，不到九成：可能换了链接器（排序清单是 lld 的参数），或者链接参数没生效"
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
fn functions(nm: &str) -> Vec<Function<'_>> {
    nm.lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let (address, size, kind, name) = match fields.as_slice() {
                [address, size, kind, name] => (*address, Some(*size), *kind, *name),
                [address, kind, name] => (*address, None, *kind, *name),
                _ => return None,
            };
            if !matches!(kind, "t" | "T" | "w" | "W") {
                return None;
            }
            let address = u64::from_str_radix(address, 16).ok()?;
            let size = match size {
                Some(size) => u64::from_str_radix(size, 16).ok()?,
                None => 0,
            };
            Some(Function {
                address,
                size,
                name,
            })
        })
        .collect()
}

/// 整数的百分比，向下取整。
fn percent(part: u64, whole: u64) -> u64 {
    part.saturating_mul(100).checked_div(whole).unwrap_or(0)
}

#[cfg(test)]
mod tests;
