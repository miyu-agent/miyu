//! macOS 的程序用链式修正、照最低系统 12.0 链接（施工 MO-6）：打压缩包以前、查包时读每个程序的加载命令，不是的报出来，
//! 不打包、不发。
//!
//! 链式修正（`LC_DYLD_CHAINED_FIXUPS`）让 macOS 13 起的内核读到哪一页才修正哪一页，修正过的页是干净的、不算进占用；老式的
//! 修正信息（`LC_DYLD_INFO_ONLY`）让 dyld 启动时把带指针的只读数据整段改写成脏页。苹果的链接器照最低系统选用哪一种：11.0
//! 是老式的，12.0 起是链式的。最低系统由编它的 cargo 环境里的 `MACOSX_DEPLOYMENT_TARGET` 定（[`crate::dist::released`]），
//! 哪一处编译漏带了，那个程序就还是 11.0、老式修正：这一项防的就是它。CI 的机器和本地的 Xcode 版本不一样，链接器的默认
//! 行为也可能不一样，所以照编出来的程序查，不照命令查。
//!
//! 加载命令照 Mach-O 格式自己读（64 位、小端，只看命令的种类和最低系统），不靠机器上的 `otool`：同 CRT-1 读导入表，查包在
//! 哪台机器上都是同一份代码，Linux 上也测得到。

use std::fs;
use std::path::Path;

use crate::dist::MACOS_MINIMUM;

/// 64 位 Mach-O 文件开头的魔数（`MH_MAGIC_64`），按小端读。
const MAGIC: u32 = 0xfeed_facf;
/// 多种架构合在一起的文件（`FAT_MAGIC`、`FAT_MAGIC_64`）开头的魔数，按小端读出来的样子。
const FAT: [u32; 2] = [0xbeba_feca, 0xbfba_feca];
/// 文件头的长度（`mach_header_64`），加载命令紧跟在它后面；命令的条数在第 16 个字节起。
const HEADER: usize = 32;
/// 记平台和最低系统的加载命令，最低系统在第 12 个字节起。
const BUILD_VERSION: u32 = 0x32;
/// 老一代记最低系统的加载命令，最低系统在第 8 个字节起。rustc 给 arm64 不写它，读到了也认。
const VERSION_MIN_MACOSX: u32 = 0x24;
/// 老式的修正信息：`LC_DYLD_INFO`、`LC_DYLD_INFO_ONLY`。
const LEGACY: [u32; 2] = [0x22, 0x8000_0022];
/// 链式修正：`LC_DYLD_CHAINED_FIXUPS`。
const CHAINED: u32 = 0x8000_0034;

/// 从一个程序的加载命令里读出来的。
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Loaded {
    /// 有 `LC_DYLD_CHAINED_FIXUPS`。
    pub(super) chained: bool,
    /// 有 `LC_DYLD_INFO_ONLY` 或者 `LC_DYLD_INFO`。
    pub(super) legacy: bool,
    /// 最低系统，照 Mach-O 的写法：`主版本 << 16 | 次版本 << 8 | 修订号`。两种记最低系统的命令都没有的是 `None`。
    pub(super) minimum: Option<u32>,
}

/// 包里的程序 `file`（报的时候叫 `shown`）不是链式修正、最低系统不是 [`MACOS_MINIMUM`] 的，交回一句问题；读不了、不是
/// 64 位 Mach-O 文件的也是问题。
pub(super) fn problem(file: &Path, shown: &str) -> Option<String> {
    let bytes = match fs::read(file) {
        Ok(bytes) => bytes,
        Err(error) => return Some(format!("{shown} 读不了（{error}）")),
    };
    let loaded = match commands(&bytes) {
        Ok(loaded) => loaded,
        Err(error) => return Some(format!("{shown} 的加载命令读不了：{error}")),
    };
    let mut wrong = Vec::new();
    if !loaded.chained {
        wrong.push("没有 LC_DYLD_CHAINED_FIXUPS（链式修正）".to_string());
    }
    if loaded.legacy {
        wrong.push("有 LC_DYLD_INFO_ONLY（老式修正）".to_string());
    }
    if loaded.minimum.is_none() || loaded.minimum != packed(MACOS_MINIMUM) {
        let minimum = loaded.minimum.map_or_else(|| "没写".to_string(), version);
        wrong.push(format!("最低系统是 {minimum}，该是 {MACOS_MINIMUM}"));
    }
    (!wrong.is_empty()).then(|| {
        format!(
            "{shown} {}：编它的 cargo 要带 MACOSX_DEPLOYMENT_TARGET={MACOS_MINIMUM}（xtask/src/dist.rs 的 released）",
            wrong.join("；")
        )
    })
}

/// 64 位 Mach-O 文件 `bytes` 的加载命令里有没有两种修正信息、最低系统是多少。
///
/// # Errors
///
/// 不是 64 位的 Mach-O 文件（包括多种架构合在一起的）、文件截断了、某条命令的长度不对。
pub(super) fn commands(bytes: &[u8]) -> Result<Loaded, String> {
    let short = || "文件截断了".to_string();
    match u32_at(bytes, 0) {
        Some(MAGIC) => {}
        Some(magic) if FAT.contains(&magic) => {
            return Err("是几种架构合在一起的文件，发行包里只该有 arm64 一种".to_string());
        }
        _ => return Err("不是 64 位的 Mach-O 文件".to_string()),
    }
    let count = u32_at(bytes, 16).ok_or_else(short)?;
    let mut loaded = Loaded {
        chained: false,
        legacy: false,
        minimum: None,
    };
    let mut at = HEADER;
    for _ in 0..count {
        let kind = u32_at(bytes, at).ok_or_else(short)?;
        let size = u32_at(bytes, at + 4).ok_or_else(short)? as usize;
        // 每条至少有种类和长度两格；长度不够的往下读会原地打转。
        if size < 8 {
            return Err(format!("第 {at} 字节起的加载命令长度是 {size}，不对"));
        }
        let end = at.checked_add(size).ok_or_else(short)?;
        if end > bytes.len() {
            return Err(short());
        }
        match kind {
            CHAINED => loaded.chained = true,
            kind if LEGACY.contains(&kind) => loaded.legacy = true,
            BUILD_VERSION => loaded.minimum = Some(u32_at(bytes, at + 12).ok_or_else(short)?),
            VERSION_MIN_MACOSX => loaded.minimum = Some(u32_at(bytes, at + 8).ok_or_else(short)?),
            _ => {}
        }
        at = end;
    }
    Ok(loaded)
}

/// `12.0`、`12.2.1` 这样写的系统版本换成 Mach-O 的写法（见 [`Loaded::minimum`]）。写法不对、某一段放不下的是 `None`。
pub(super) fn packed(text: &str) -> Option<u32> {
    let parts: Vec<u32> = text
        .split('.')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    let (major, minor, patch) = match parts.as_slice() {
        [major] => (*major, 0, 0),
        [major, minor] => (*major, *minor, 0),
        [major, minor, patch] => (*major, *minor, *patch),
        _ => return None,
    };
    (major <= 0xffff && minor <= 0xff && patch <= 0xff)
        .then_some((major << 16) | (minor << 8) | patch)
}

/// Mach-O 写法的系统版本写成人看的：修订号是 0 的写两段（`11.0`），不是的写三段（`12.2.1`）。
pub(super) fn version(packed: u32) -> String {
    let (major, minor, patch) = (packed >> 16, (packed >> 8) & 0xff, packed & 0xff);
    match patch {
        0 => format!("{major}.{minor}"),
        _ => format!("{major}.{minor}.{patch}"),
    }
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

#[cfg(test)]
mod tests;
