//! 排函数的清单怎么定（施工 LW-1）：读符号表、读模拟器的日志、照头一回执行的先后记函数。都不碰文件系统：符号表是 `nm`
//! 的输出原文，日志是一个 [`BufRead`]，`order.rs` 负责拉 `nm`、开文件、写清单。
//!
//! 日志（QEMU 的 `-d in_asm`）一块的样子：
//!
//! ```text
//! ----------------
//! IN:
//! 0x7faae70eb7e0:  48 89 e7                 movq     %rsp, %rdi
//! 0x7faae70eb7e3:  e8 a8 0c 00 00           callq    0x7faae70ec490
//! ```
//!
//! QEMU 没有带反汇编库的（Ubuntu 24.04 的 `qemu-user` 就是）写成另一种：一块只有一行地址，后面跟着这一块的字节原文。
//!
//! ```text
//! ----------------
//! IN:
//! 0x2aaaab2cc1d0:
//! OBJD-T: f30f1efa55488d0d24fefdff488d05f590010066480f6ec966480f6ed0660f6c
//! OBJD-T: 31
//! ```
//!
//! 两种都认（[`address`]）：行首 `0x<地址>:` 后面两个空格，再是一个字节（带反汇编的，一条指令一行），或者到行尾什么都没有
//! （不带反汇编的，只有一块的起点）。只有起点也够用：认的是这一块落在哪个函数里。地址是模拟器里的绝对地址，减去程序的
//! 装载起点才是符号表里的；减出来不在哪个函数的范围里的（动态库、模拟器自己、两个函数之间的空隙）不算。几条线程的块交错
//! 着写，但一行不会被劈开，照行读就行。

use std::collections::HashSet;
use std::io::{self, BufRead};

/// 符号表里的一个函数：起点、大小（字节）、原名（不解码，链接器的排序清单要的就是它）。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Function {
    start: u64,
    size: u64,
    name: String,
}

/// 程序里的函数，照起点排好。
#[derive(Debug, Default)]
pub struct Symbols {
    functions: Vec<Function>,
}

impl Symbols {
    /// 读 `nm --defined-only --numeric-sort -S` 的输出：一行 `<起点> <大小> <类型> <名字>`，类型是 `t`、`T`、`w`、`W`
    /// （代码段里的函数，弱的也算）的留下。没写大小的（汇编写的几个入口）、别的类型（数据、只读数据）跳过。
    pub fn parse(nm: &str) -> Symbols {
        let mut functions: Vec<Function> = nm
            .lines()
            .filter_map(|line| {
                let mut fields = line.splitn(4, ' ');
                let start = u64::from_str_radix(fields.next()?, 16).ok()?;
                let size = u64::from_str_radix(fields.next()?, 16).ok()?;
                let kind = fields.next()?;
                let name = fields.next()?;
                matches!(kind, "t" | "T" | "w" | "W").then(|| Function {
                    start,
                    size,
                    name: name.to_string(),
                })
            })
            .collect();
        // `--numeric-sort` 本来就排好了；再排一遍（稳定的），不靠 `nm` 的参数。
        functions.sort_by_key(|function| function.start);
        Symbols { functions }
    }

    /// 几个函数。
    pub fn count(&self) -> usize {
        self.functions.len()
    }

    /// 符号表里的地址 `address` 落在哪个函数里：起点不比它大的最后一个，地址在它的范围里（大小是 0 的当一个字节）。
    fn find(&self, address: u64) -> Option<usize> {
        let index = self
            .functions
            .partition_point(|function| function.start <= address)
            .checked_sub(1)?;
        let function = &self.functions[index];
        (address < function.start + function.size.max(1)).then_some(index)
    }
}

/// 日志里一行上的地址；不是带地址的行的是 `None`。两种写法都认（见模块的说明）：带反汇编的，一条指令一行，地址后面是这条
/// 指令的字节；不带反汇编的，一块只有一行，地址后面什么都没有。
pub fn address(line: &[u8]) -> Option<u64> {
    let rest = line.strip_prefix(b"0x")?;
    let digits = rest.iter().take_while(|c| c.is_ascii_hexdigit()).count();
    let after = rest[digits..].strip_prefix(b":  ")?;
    let bare = after.iter().all(u8::is_ascii_whitespace);
    let byte = after.len() >= 3 && after[..2].iter().all(u8::is_ascii_hexdigit) && after[2] == b' ';
    if digits == 0 || !(bare || byte) {
        return None;
    }
    let text = std::str::from_utf8(&rest[..digits]).ok()?;
    u64::from_str_radix(text, 16).ok()
}

/// 执行过的函数，照头一回执行的先后。几段日志接着读：后一段里新出现的接在后面。
pub struct Listing<'a> {
    symbols: &'a Symbols,
    seen: Vec<bool>,
    order: Vec<usize>,
}

impl<'a> Listing<'a> {
    /// 空的，函数照 `symbols` 认。
    pub fn new(symbols: &'a Symbols) -> Listing<'a> {
        Listing {
            symbols,
            seen: vec![false; symbols.count()],
            order: Vec::new(),
        }
    }

    /// 照先后读一段日志 `log`，程序在这一段里装在 `base`：一行一行地读、边读边认，不整份读进来（一段几十 MB）。
    ///
    /// # Errors
    ///
    /// 读不下去。
    pub fn read(&mut self, base: u64, mut log: impl BufRead) -> io::Result<()> {
        let mut line = Vec::new();
        while log.read_until(b'\n', &mut line)? > 0 {
            if let Some(at) = address(&line).and_then(|at| at.checked_sub(base))
                && let Some(index) = self.symbols.find(at)
                && !self.seen[index]
            {
                self.seen[index] = true;
                self.order.push(index);
            }
            line.clear();
        }
        Ok(())
    }

    /// 清单：函数的原名照先后，一个名字只出现一次（不同的地方有同名的局部函数时，链接器照名字排，写一次就够）。
    pub fn names(&self) -> Vec<&'a str> {
        let mut written = HashSet::new();
        self.order
            .iter()
            .map(|&index| self.symbols.functions[index].name.as_str())
            .filter(|name| written.insert(*name))
            .collect()
    }

    /// 执行过的函数一共多少字节（整个函数的大小，不是执行过的那几块）。
    pub fn bytes(&self) -> u64 {
        self.order
            .iter()
            .map(|&index| self.symbols.functions[index].size)
            .sum()
    }
}

#[cfg(test)]
mod tests;
