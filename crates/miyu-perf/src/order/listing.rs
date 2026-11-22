//! 排函数的清单怎么定（施工 LW-1）：读符号表、读模拟器的日志、照头一回执行的先后记函数；macOS 上另读页探针的记录（施工
//! MO-1，下面「页探针的记录」）。都不碰文件系统：符号表是 `nm` 的输出原文，日志是一个 [`BufRead`]，探针的记录是文件的
//! 字节，`order.rs` 负责拉 `nm`、开文件、写清单。
//!
//! 符号表在两个系统上不一样：GNU 的 `nm -S` 给每个函数写大小；苹果的 `nm`（llvm-nm）读 Mach-O 时 `-S` 那一列全是 0
//! （Mach-O 的符号没有大小）。整张表里列出的函数大小都是 0 时（[`Symbols::parse`]），每个函数的大小改用「下一个符号的
//! 起点减去自己的起点」，下一个符号是任何类型的都算（代码段后面接着的是别的节里的符号）；后面一个符号都没有的，给它
//! [`LAST`] 字节；苹果列成 `T` 的文件头（[`HEADER`]）不算函数。Linux 的表里总有带大小的函数，那里大小是 0 的是汇编
//! 写的空标记，仍然当一个字节，行为和以前一样。`xtask/src/order/verify.rs` 的 `functions` 用同一个算法补大小，改一边要
//! 改另一边。名字照原样留着：Mach-O 的符号开头多一个下划线，这里不去掉（地址换成符号用不到名字）。
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
//!
//! 页探针的记录（`crates/miyu-perf/probe/pagetrace.c`，格式和它开头的注释一致，改一边要改另一边）：都是小端的 64 位
//! 无符号数。开头 8 个：魔数（字节是 `MIYUPGT1`）、`__TEXT` 段在符号表里的虚拟地址、页大小、`__text` 节在符号表里的
//! 起点、`__text` 节的大小、记录的上限、记录数、标志（第 0 位是「满了」，别的位是 0）。后面接着记录，每条两个数：碰的
//! 地址（已经是符号表里的地址，不用再减装载起点）、种类（1 是执行，2 是读）。同一个地址探针只记一次，照第一次见到的
//! 先后排。[`Probe::parse`] 只收「执行」的记录；文件是空的、截断了、多出字节、魔数不对、记录数超过上限、标志或种类
//! 不认得的都报错；标了「满了」的照样读，另外交回 [`Probe::full`]，调用方报一句提醒。

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

/// 苹果的符号表里，最后一个函数后面一个符号都没有时给它的大小：一页（Apple 芯片上是 16 kB）。拿不到 `__text` 的末尾
/// （`nm` 不列节的范围），这里只求不把它后面的地址认成它；真实的程序里代码段后面总还有别的节的符号，用不到这一条。
pub const LAST: u64 = 16 * 1024;

/// Mach-O 的文件头：苹果的 `nm` 把它列成 `T`，地址是 `__TEXT` 段的起点、在第一个函数前面，它不是函数，不留。Linux 的
/// 符号表里没有这个名字。
const HEADER: &str = "__mh_execute_header";

impl Symbols {
    /// 读 `nm --defined-only --numeric-sort -S` 的输出：一行 `<起点> <大小> <类型> <名字>`，类型是 `t`、`T`、`w`、`W`
    /// （代码段里的函数，弱的也算）的留下。没写大小的（汇编写的几个入口）、别的类型（数据、只读数据）、Mach-O 的文件头
    /// 跳过。
    ///
    /// 留下的函数大小全是 0 的（苹果的 `nm`，见模块的说明），大小改用下一个符号（任何类型）的起点减去自己的起点，起点相同
    /// 的几个符号（别名）一样大；最后一个后面没有符号的是 [`LAST`]。
    pub fn parse(nm: &str) -> Symbols {
        let rows: Vec<(u64, u64, &str, &str)> = nm
            .lines()
            .filter_map(|line| {
                let mut fields = line.splitn(4, ' ');
                let start = u64::from_str_radix(fields.next()?, 16).ok()?;
                let size = u64::from_str_radix(fields.next()?, 16).ok()?;
                Some((start, size, fields.next()?, fields.next()?))
            })
            .collect();
        let mut functions: Vec<Function> = rows
            .iter()
            .filter(|&&(_, _, kind, name)| matches!(kind, "t" | "T" | "w" | "W") && name != HEADER)
            .map(|&(start, size, _, name)| Function {
                start,
                size,
                name: name.to_string(),
            })
            .collect();
        // `--numeric-sort` 本来就排好了；再排一遍（稳定的），不靠 `nm` 的参数。
        functions.sort_by_key(|function| function.start);
        if !functions.is_empty() && functions.iter().all(|function| function.size == 0) {
            let mut starts: Vec<u64> = rows.iter().map(|&(start, ..)| start).collect();
            starts.sort_unstable();
            starts.dedup();
            for function in &mut functions {
                let next = starts.partition_point(|&start| start <= function.start);
                function.size = starts
                    .get(next)
                    .map_or(LAST, |&start| start - function.start);
            }
        }
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
            if let Some(at) = address(&line).and_then(|at| at.checked_sub(base)) {
                self.note(at);
            }
            line.clear();
        }
        Ok(())
    }

    /// 照先后记一串符号表里的地址（页探针的记录，[`Probe::executed`]）：落在哪个函数里，那个函数第一次出现时接在后面。
    /// 和 [`Listing::read`] 记的是同一份，几份记录、几段日志可以接着记。
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "施工 MO-2 接上 --probe 以前只有测试用")
    )]
    pub fn add(&mut self, addresses: &[u64]) {
        for &at in addresses {
            self.note(at);
        }
    }

    /// 符号表里的地址 `at` 落在的函数要是第一次出现，接在后面。
    fn note(&mut self, at: u64) {
        if let Some(index) = self.symbols.find(at)
            && !self.seen[index]
        {
            self.seen[index] = true;
            self.order.push(index);
        }
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

/// 读好的一份页探针的记录（格式见模块的说明）。
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "施工 MO-2 接上 --probe 以前只有测试用")
)]
pub struct Probe {
    /// 「执行」的记录碰的地址，照记录的先后；「读」的不在这里。
    pub executed: Vec<u64>,
    /// 一共几条记录（执行的加读的）。
    pub records: usize,
    /// 探针记满了：之后碰到的新地址没有记下来，清单可能缺一截，调用方要报一句。
    pub full: bool,
}

/// 记录文件的魔数：字节是 `MIYUPGT1`。
const MAGIC: [u8; 8] = *b"MIYUPGT1";

/// 文件头是几个 64 位数。
const HEAD: usize = 8;

/// 标志里认得的位：第 0 位是「满了」。
const FULL: u64 = 1;

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "施工 MO-2 接上 --probe 以前只有测试用")
)]
impl Probe {
    /// 读一份记录文件的字节 `bytes`。
    ///
    /// # Errors
    ///
    /// 文件是空的、比文件头短、比文件头说的记录数短（截断了）或者长（多出字节）、魔数不对、记录数超过上限、标志里有
    /// 不认得的位、某条记录的种类不是 1 也不是 2：交回是哪一样。
    pub fn parse(bytes: &[u8]) -> Result<Probe, String> {
        if bytes.is_empty() {
            return Err("探针的记录是空文件".to_string());
        }
        let words: Vec<u64> = bytes
            .as_chunks::<8>()
            .0
            .iter()
            .map(|chunk| u64::from_le_bytes(*chunk))
            .collect();
        let Some(&[magic, _, _, _, _, capacity, records, flags]) = words.first_chunk::<HEAD>()
        else {
            return Err(format!(
                "探针的记录截断了：只有 {} 字节，文件头要 {} 字节",
                bytes.len(),
                HEAD * 8
            ));
        };
        if magic.to_le_bytes() != MAGIC {
            return Err(format!("不是探针的记录：魔数是 {magic:#018x}"));
        }
        if flags & !FULL != 0 {
            return Err(format!("探针的记录有不认得的标志 {flags:#x}"));
        }
        if records > capacity {
            return Err(format!("探针的记录数 {records} 超过了上限 {capacity}"));
        }
        let want = usize::try_from(records)
            .ok()
            .and_then(|records| records.checked_mul(16)?.checked_add(HEAD * 8))
            .ok_or_else(|| format!("探针的记录数 {records} 大得不对"))?;
        if bytes.len() != want {
            let how = match bytes.len() < want {
                true => "截断了",
                false => "多出了字节",
            };
            return Err(format!(
                "探针的记录{how}：文件头说 {records} 条、应该 {want} 字节，文件是 {} 字节",
                bytes.len()
            ));
        }
        let mut executed = Vec::new();
        for (index, &[address, kind]) in words[HEAD..].as_chunks::<2>().0.iter().enumerate() {
            match kind {
                1 => executed.push(address),
                2 => {}
                other => {
                    return Err(format!(
                        "探针的第 {} 条记录种类是 {other}，不认得",
                        index + 1
                    ));
                }
            }
        }
        Ok(Probe {
            executed,
            records: (words.len() - HEAD) / 2,
            full: flags & FULL != 0,
        })
    }
}

#[cfg(test)]
mod tests;
