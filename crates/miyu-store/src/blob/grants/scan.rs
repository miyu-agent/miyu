//! 在一个包自己的状态目录里找它记着的 blob 哈希（施工 BG-1补，`docs/construction/BG-1-包存的图重起后读得到.md`「补登记」）：
//! BG-1 之前存的 blob，核心从来没记过是谁存的；表情包这样的包把图只存在核心里、自己的状态目录 `var/lib/<编号>/` 里只记
//! 哈希。升级后第一次起来（数据根里还没有 `var/lib/blob-grants/`），核心照这里找到的补登记给它。
//!
//! 只做那一次，这里不管、调的一方管（端点的 `attach/grants/backfill.rs`）：包以前能把哈希记进自己的状态，说明它当时拿得到；
//! BG-1 以后许可都落盘了，不用再从状态目录认。要是每次起来都扫，包就能靠往自己目录里写别处看来的哈希给自己开门。
//!
//! 怎么找：普通文件照字节找 `sha256:` 后面跟 64 位小写十六进制（[`ContentHash`] 的写法，照它的解析认），二进制的也这样找
//! （SQLite 库里存着的）。不跟链接、不出这个目录：上一层 `var/lib/` 换成真实的位置（数据根本身可能经过一层链接，同
//! [`Blobs::read_range`](crate::blob::Blobs::read_range) 只换目录），状态目录本身是链接的不扫，里面的链接跳过，文件照
//! [`miyu_fs::open_file`] 打开（路上一层链接都不跟）。有上限（[`Limits`]）：太大的文件跳过，文件太多、目录太深的不往下扫，
//! 都数在 [`Scanned`] 里，调的一方记一行。存处里有没有这个 blob 这里不查。

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use miyu_kernel::id::ContentHash;

#[cfg(test)]
mod tests;

/// 哈希打头的几个字。
const PREFIX: &[u8] = b"sha256:";

/// 一个哈希写出来多长：`sha256:` 加 64 位。
const LENGTH: usize = PREFIX.len() + 64;

/// 一次读多少。
const CHUNK: usize = 64 * 1024;

/// 扫一个包的状态目录的上限。
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// 一份文件最多多少字节，再大的跳过。
    pub file_bytes: u64,
    /// 最多扫多少份文件。
    pub files: usize,
    /// 状态目录下面最多进几层子目录。
    pub depth: usize,
}

impl Default for Limits {
    /// 64 MiB、20000 份、16 层（2026-11-11 项目主人定）。
    fn default() -> Limits {
        Limits {
            file_bytes: 64 * 1024 * 1024,
            files: 20_000,
            depth: 16,
        }
    }
}

/// 扫出来的。
#[derive(Debug, Default)]
pub struct Scanned {
    /// 找到的哈希。
    pub found: BTreeSet<ContentHash>,
    /// 扫了几份文件（太大跳过的、读不了的也算）。
    pub files: usize,
    /// 太大跳过的文件。
    pub too_large: usize,
    /// 太深没进去的目录。
    pub too_deep: usize,
    /// 列不出来的目录、读不了的文件。
    pub unreadable: usize,
    /// 文件到了上限，后面的没扫。
    pub cut_short: bool,
}

impl Scanned {
    /// 是不是整个扫完了：没有跳过的、没进去的、读不了的。
    pub fn complete(&self) -> bool {
        !self.cut_short && self.too_large == 0 && self.too_deep == 0 && self.unreadable == 0
    }
}

/// 扫包的状态目录 `dir`（`DataRoot::package_state`），交回里面记着的哈希。目录没有的是空的。
///
/// # Errors
///
/// `dir` 没有上一层；上一层换不成真实的位置；`dir` 在、却不是目录（是链接、是文件）；`dir` 本身列不出来。
pub fn scan(dir: &Path, limits: &Limits) -> io::Result<Scanned> {
    let mut scanned = Scanned::default();
    let (Some(parent), Some(name)) = (dir.parent(), dir.file_name()) else {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    };
    let real = match fs::canonicalize(parent) {
        Ok(parent) => parent.join(name),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(scanned),
        Err(error) => return Err(error),
    };
    match fs::symlink_metadata(&real) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => return Err(io::Error::other("not a directory (a link or a file)")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(scanned),
        Err(error) => return Err(error),
    }
    let mut pending = vec![(real, 0usize)];
    while let Some((dir, depth)) = pending.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if depth == 0 => return Err(error),
            Err(_) => {
                scanned.unreadable += 1;
                continue;
            }
        };
        for entry in entries {
            // 列到一半出错的：这个目录剩下的不知道还有没有，算没扫完。
            let Ok(entry) = entry else {
                scanned.unreadable += 1;
                break;
            };
            let Ok(kind) = entry.file_type() else {
                scanned.unreadable += 1;
                continue;
            };
            if kind.is_dir() {
                if depth < limits.depth {
                    pending.push((entry.path(), depth + 1));
                } else {
                    scanned.too_deep += 1;
                }
            } else if kind.is_file() {
                if scanned.files >= limits.files {
                    scanned.cut_short = true;
                    return Ok(scanned);
                }
                scanned.files += 1;
                read_one(&entry.path(), limits, &mut scanned);
            }
            // 链接、FIFO、套接字这些不碰。
        }
    }
    Ok(scanned)
}

/// 扫一份文件：太大的跳过；列出来以后被换成链接、别的东西的，读不了的，都算读不了。
fn read_one(path: &Path, limits: &Limits, scanned: &mut Scanned) {
    let opened = miyu_fs::open_file(path)
        .map_err(io::Error::other)
        .and_then(|file| Ok((file.metadata()?.len(), file)));
    let Ok((size, file)) = opened else {
        scanned.unreadable += 1;
        return;
    };
    if size > limits.file_bytes {
        scanned.too_large += 1;
        return;
    }
    // 量过以后还在长的，也只读到上限。
    if hashes_in(file.take(limits.file_bytes), &mut scanned.found).is_err() {
        scanned.unreadable += 1;
    }
}

/// 一块一块读 `reader`，照字节找哈希：上一块末尾留下不够一个哈希长的那一截，接在下一块前面，跨着两块的也找得到。
fn hashes_in(mut reader: impl Read, found: &mut BTreeSet<ContentHash>) -> io::Result<()> {
    let mut buffer = vec![0u8; CHUNK + LENGTH];
    let mut kept = 0;
    loop {
        let read = match reader.read(&mut buffer[kept..]) {
            Ok(0) => return Ok(()),
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        let filled = kept + read;
        collect(&buffer[..filled], found);
        kept = filled.min(LENGTH - 1);
        buffer.copy_within(filled - kept..filled, 0);
    }
}

/// `bytes` 里每一处 `sha256:` 后面跟着的合写法的哈希。
fn collect(bytes: &[u8], found: &mut BTreeSet<ContentHash>) {
    let mut from = 0;
    while let Some(at) = bytes[from..]
        .windows(PREFIX.len())
        .position(|window| window == PREFIX)
    {
        let start = from + at;
        let parsed = bytes
            .get(start..start + LENGTH)
            .and_then(|text| std::str::from_utf8(text).ok())
            .and_then(|text| ContentHash::parse(text).ok());
        found.extend(parsed);
        from = start + PREFIX.len();
    }
}
