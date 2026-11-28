//! Windows 的程序不要 VC++ 运行库（施工 CRT-1）：查包时读每个 `.exe` 的导入表，要了 `VCRUNTIME*.dll`、`MSVCP*.dll` 的报出来，
//! 这一趟不发。
//!
//! 这两种 DLL 是装 VC++ 2015–2022 运行库时放进系统的，Windows 自己不带：没装过的机器上程序报「找不到 VCRUNTIME140.dll」、
//! 起不来。rc.5 的五个 `.exe` 都要 `VCRUNTIME140.dll`，GitHub 的 Windows 机器本来装着运行库，install-test 照样过，所以要在
//! 查包这里拦。`api-ms-win-crt-*`（通用 C 运行库，Windows 10 起系统自带）、`msvcrt.dll`（系统的）不算。
//!
//! 怎么不要：仓库的 `.cargo/config.toml` 给 MSVC 目标加 `+crt-static`，把 C 运行库静态链进程序；两个头在各自的检出里编，
//! `release.yml` 用 `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS` 也给它们加上。这一项防的是哪一处漏了。
//!
//! 导入表照 PE 格式自己读（只读普通导入表里的 DLL 名字），不靠机器上的 dumpbin、llvm-readobj：查包在哪台机器上都是同一份
//! 代码，Linux 上也测得到。

use std::fs;
use std::path::Path;

/// 要了它就是要装 VC++ 运行库的 DLL 名字开头，比的时候不分大小写：`vcruntime140.dll`、`vcruntime140_1.dll`、`msvcp140.dll`
/// 这些。
const RUNTIME: [&str; 2] = ["vcruntime", "msvcp"];

/// 一条导入描述符的长度（`IMAGE_IMPORT_DESCRIPTOR`），DLL 名字的地址在第 12 个字节起。
const DESCRIPTOR: usize = 20;
/// 节表里一项的长度（`IMAGE_SECTION_HEADER`）。
const SECTION: usize = 40;

/// 包里的程序 `file`（报的时候叫 `shown`）要了 VC++ 运行库的，交回一句问题；读不了、不是 PE 文件的也是问题。
pub(super) fn problem(file: &Path, shown: &str) -> Option<String> {
    let bytes = match fs::read(file) {
        Ok(bytes) => bytes,
        Err(error) => return Some(format!("{shown} 读不了（{error}）")),
    };
    let dlls = match imports(&bytes) {
        Ok(dlls) => dlls,
        Err(error) => return Some(format!("{shown} 的导入表读不了：{error}")),
    };
    let runtime: Vec<&str> = dlls
        .iter()
        .map(String::as_str)
        .filter(|dll| needs_runtime(dll))
        .collect();
    (!runtime.is_empty()).then(|| {
        format!(
            "{shown} 要 {}：没装 VC++ 运行库的机器上起不来（MSVC 目标要带 -C target-feature=+crt-static）",
            runtime.join("、")
        )
    })
}

/// `dll` 是 VC++ 运行库的。
pub(super) fn needs_runtime(dll: &str) -> bool {
    let lower = dll.to_ascii_lowercase();
    RUNTIME.iter().any(|prefix| lower.starts_with(prefix))
}

/// PE 文件 `bytes` 的导入表里的 DLL 名字，照表里的先后。没有导入表的是空的。
///
/// # Errors
///
/// 不是 PE 文件、文件截断了、导入表或名字指到哪一节都不在的地方，交回原因。
pub(super) fn imports(bytes: &[u8]) -> Result<Vec<String>, String> {
    let short = || "文件截断了".to_string();
    if bytes.get(..2) != Some(b"MZ".as_slice()) {
        return Err("不是 PE 文件（开头不是 MZ）".to_string());
    }
    let pe = u32_at(bytes, 0x3c).ok_or_else(short)? as usize;
    if bytes.get(pe..pe + 4) != Some(b"PE\0\0".as_slice()) {
        return Err("不是 PE 文件（没有 PE 签名）".to_string());
    }
    let coff = pe + 4;
    let sections = usize::from(u16_at(bytes, coff + 2).ok_or_else(short)?);
    let optional_size = usize::from(u16_at(bytes, coff + 16).ok_or_else(short)?);
    let optional = coff + 20;
    // 数据目录前面紧挨着它的项数（NumberOfRvaAndSizes）；32 位、64 位的可选头长短不一样。
    let directories = match u16_at(bytes, optional).ok_or_else(short)? {
        0x10b => optional + 96,
        0x20b => optional + 112,
        other => return Err(format!("不认识的可选头（{other:#x}）")),
    };
    // 第二项是导入表。
    if u32_at(bytes, directories - 4).ok_or_else(short)? < 2 {
        return Ok(Vec::new());
    }
    let table = u32_at(bytes, directories + 8).ok_or_else(short)?;
    if table == 0 {
        return Ok(Vec::new());
    }
    let section_table = optional + optional_size;
    // 内存里的地址换成文件里的位置：照它落在哪一节。
    let offset = |rva: u32| {
        (0..sections).find_map(|index| {
            let at = section_table + index * SECTION;
            let size = u32_at(bytes, at + 8)?.max(u32_at(bytes, at + 16)?);
            let start = u32_at(bytes, at + 12)?;
            let raw = u32_at(bytes, at + 20)?;
            let inside = rva.checked_sub(start).filter(|inside| *inside < size)?;
            Some(raw as usize + inside as usize)
        })
    };
    let mut at = offset(table).ok_or("导入表不在任何一节里")?;
    let mut names = Vec::new();
    loop {
        let entry = bytes.get(at..at + DESCRIPTOR).ok_or_else(short)?;
        if entry.iter().all(|byte| *byte == 0) {
            return Ok(names);
        }
        let name = u32_at(bytes, at + 12).ok_or_else(short)?;
        let start = offset(name).ok_or("DLL 的名字不在任何一节里")?;
        let rest = bytes.get(start..).ok_or_else(short)?;
        let end = rest
            .iter()
            .position(|byte| *byte == 0)
            .ok_or("DLL 的名字没有结尾")?;
        names.push(String::from_utf8_lossy(rest.get(..end).ok_or_else(short)?).into_owned());
        at += DESCRIPTOR;
    }
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

#[cfg(test)]
mod tests;
