//! 读一段的页探针记录（施工 MO-2，macOS 上代替模拟器的日志）：探针给这一段的每个进程各写一份 `<前缀>.<进程号>`，旁边的
//! `<前缀>.<进程号>.exe` 是一行旁注：可执行文件的路径和前两个参数，用空格隔开（格式见 `crates/miyu-perf/probe/pagetrace.c`
//! 开头的注释，记录文件怎么读见 `order/listing.rs` 的 [`Probe::parse`]）。
//!
//! 一段里可能有几个进程的文件：探针照 `MIYU_PAGES_PROGRAM` 只在被测程序里动手，可是被测程序自己也可能再起一份自己。这里只读
//! 旁注里写着被测程序的那几份，照进程号从小到大接着交给 [`Listing`]（进程号大致就是起来的先后），每一份里照记录的先后。
//! 探针改名落盘时留下的 `.tmp`、别的段的前缀都不算。

use std::path::{Path, PathBuf};

use super::listing::{Listing, Probe};

/// 一段读下来的：几个进程的记录、一共几条（执行的加读的）、其中「执行」的几条。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Read {
    /// 读了几个进程的记录文件。
    pub files: usize,
    /// 一共几条记录。
    pub records: usize,
    /// 其中「执行」的几条：交给清单的就是这些。
    pub executed: usize,
}

/// 读前缀 `prefix` 这一段里被测程序 `program` 的记录，照先后交给 `listing`。记满了的照样读，另在标准错误上提醒一句。
///
/// # Errors
///
/// 目录读不了、被测程序找不到、某份记录或旁注读不了、记录的格式不对；一份被测程序的记录都没有，或者「执行」的记录一条都
/// 没有（报错里写可能的原因：程序带了加固运行时，系统不让插库）。
pub fn read(prefix: &Path, program: &Path, listing: &mut Listing<'_>) -> Result<Read, String> {
    let wanted = program
        .canonicalize()
        .map_err(|e| format!("找不到 {}：{e}", program.display()))?;
    let mut read = Read::default();
    let mut others = 0;
    for (_, file) in files(prefix)? {
        let note_file = side(&file);
        let note = match std::fs::read_to_string(&note_file) {
            Ok(note) => note,
            // 旁注在记录以前写：没有旁注的不是一份完整的记录，不算被测程序的。
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                others += 1;
                continue;
            }
            Err(e) => return Err(format!("读不了 {}：{e}", note_file.display())),
        };
        if !names(&note, &wanted) {
            others += 1;
            continue;
        }
        let bytes = std::fs::read(&file).map_err(|e| format!("读不了 {}：{e}", file.display()))?;
        let probe = Probe::parse(&bytes).map_err(|e| format!("{}：{e}", file.display()))?;
        if probe.full {
            eprintln!(
                "排函数：探针的记录记满了（{}，{} 条）：之后碰到的新地址没有记下来，清单可能缺一截",
                file.display(),
                probe.records
            );
        }
        listing.add(&probe.executed);
        read.files += 1;
        read.records += probe.records;
        read.executed += probe.executed.len();
    }
    let why = "可能的原因：程序带了加固运行时（hardened runtime），或者系统不让插库（DYLD_INSERT_LIBRARIES 没有生效）";
    if read.files == 0 {
        return Err(format!(
            "探针没有写下 {} 的记录（{}.<进程号>，别的进程的 {others} 份不算）：{why}",
            wanted.display(),
            prefix.display()
        ));
    }
    if read.executed == 0 {
        return Err(format!(
            "探针给 {} 写了 {} 份记录，可是「执行」的记录一条都没有（{}.<进程号>）：{why}",
            wanted.display(),
            read.files,
            prefix.display()
        ));
    }
    Ok(read)
}

/// 删掉前缀 `prefix` 这一段探针写的全部文件：记录、旁注、没改完名的 `.tmp`，别的进程的也删。别的段的不碰。
///
/// # Errors
///
/// 目录读不了、哪一份删不掉。
pub fn remove(prefix: &Path) -> Result<(), String> {
    let (dir, stem) = split(prefix)?;
    let lead = format!("{stem}.");
    for entry in entries(&dir)? {
        if entry
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(&lead))
        {
            std::fs::remove_file(&entry).map_err(|e| format!("删不掉 {}：{e}", entry.display()))?;
        }
    }
    Ok(())
}

/// 前缀 `prefix` 这一段的记录文件（`<前缀>.<进程号>`），照进程号从小到大。
fn files(prefix: &Path) -> Result<Vec<(u32, PathBuf)>, String> {
    let (dir, stem) = split(prefix)?;
    let mut files: Vec<(u32, PathBuf)> = entries(&dir)?
        .into_iter()
        .filter_map(|path| {
            let pid = pid_of(path.file_name()?.to_str()?, &stem)?;
            Some((pid, path))
        })
        .collect();
    files.sort_unstable();
    Ok(files)
}

/// 前缀所在的目录和前缀的文件名。
fn split(prefix: &Path) -> Result<(PathBuf, String), String> {
    let stem = prefix
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("探针的输出前缀 {} 不是一个文件名", prefix.display()))?;
    let dir = match prefix.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
        _ => PathBuf::from("."),
    };
    Ok((dir, stem.to_string()))
}

/// 目录 `dir` 里的全部条目。
fn entries(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let read = |e: std::io::Error| format!("读不了 {}：{e}", dir.display());
    std::fs::read_dir(dir)
        .map_err(read)?
        .map(|entry| entry.map(|entry| entry.path()).map_err(read))
        .collect()
}

/// 文件名 `name` 是不是 `<stem>.<进程号>`：是的交回进程号。旁注（`.exe`）、没改完名的（`.tmp`）、别的段的都不是。
fn pid_of(name: &str, stem: &str) -> Option<u32> {
    let digits = name.strip_prefix(stem)?.strip_prefix('.')?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    // 全是数字还解不出来的只有超出进程号范围这一种：那不是探针写的文件名，不是要报的错。
    digits.parse().ok()
}

/// 记录文件旁边的旁注：`<记录文件>.exe`。
fn side(file: &Path) -> PathBuf {
    let mut name = file.as_os_str().to_os_string();
    name.push(".exe");
    PathBuf::from(name)
}

/// 旁注 `note` 写的可执行文件是不是 `wanted`（已经解开符号链接的绝对路径）。路径里可以有空格，后面跟着最多两个参数，所以
/// 从左往右在每一个空格处切一刀，切出来的前一半（或者整行）解开以后是 `wanted` 的就算。
fn names(note: &str, wanted: &Path) -> bool {
    let line = note.strip_suffix('\n').unwrap_or(note);
    let cuts = line
        .match_indices(' ')
        .map(|(at, _)| at)
        .chain([line.len()]);
    cuts.map(|at| Path::new(&line[..at]))
        .any(|exe| exe == wanted || exe.canonicalize().is_ok_and(|resolved| resolved == wanted))
}

/// 真的探针只在 macOS 上跑得了（默认不跑，怎么跑见里面的说明）。
#[cfg(all(test, target_os = "macos"))]
mod mac;
#[cfg(test)]
mod tests;
