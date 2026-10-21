//! 包文件（施工 G-1，设计 `37-软件源.md` 第二节）：一个包文件夹打成一个 `.miyu`，zstd 压的 tar，里面最上层是 `<编号>/`，
//! `tar --zstd -xf` 解开就是那个包文件夹。
//!
//! - **可重现**：同一个文件夹打出来逐字节一样。文件照路径的字节排序，时间写 0，属主写 0，权限只留「能不能执行」：
//!   Unix 上有执行位的、`bin/` 下的（Windows 上没有执行位，包里的程序约定放 `bin/`）写 `755`，别的 `644`。压缩单线程、
//!   级别固定。
//! - **不打进去的**（施工 G-13）：`.git`、`.github`、`.hg`、`.svn`、`__pycache__`、`.DS_Store`、名字以 `.pyc` 结尾的，
//!   在哪一层都跳过，是文件夹还是文件都一样、不往里走。本地打的和包仓库的 `pack.yml` 剔过再打的是同一个包。
//! - **只有文件和文件夹**：链接、设备这些打的时候拒、解的时候也拒（解开的东西不能指到包外面去）。
//! - **解的时候查路径**：每一项都在同一个最上层文件夹里，不许绝对路径、`..`；最多 [`MOST_ENTRIES`] 项、[`MOST_BYTES`]
//!   字节（防压缩炸弹）。

use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use miyu_config::package::Target;

/// 包文件的后缀（设计 31 第八节第 1 条，2026-10-11 项目主人定）。
pub const EXTENSION: &str = "miyu";

/// 压缩的级别：发出去的包下载的人多，压得狠一点，打包慢一点不要紧。
const LEVEL: i32 = 19;

/// 在哪一层都不打进去的名字（施工 G-13）：版本库、Python 的缓存、macOS 的文件夹信息。照名字比，分大小写。
const SKIPPED: [&str; 6] = [".git", ".github", ".hg", ".svn", "__pycache__", ".DS_Store"];

/// 名字以它结尾的也不打进去：Python 编好的字节码（施工 G-13）。
const SKIPPED_SUFFIX: &str = ".pyc";

/// 解开最多几项。
pub const MOST_ENTRIES: usize = 100_000;

/// 解开最多多少字节：内置的语义模型那种几十 MB 的远远够。
pub const MOST_BYTES: u64 = 4 << 30;

/// 打包、解包不成的原因。
#[derive(Debug)]
pub enum Error {
    /// 读写不成。
    Io(io::Error),
    /// 有链接、设备这些不是文件也不是文件夹的（相对包文件夹的路径）。
    NotPlain(String),
    /// 路径不对：名字不是 UTF-8、绝对路径、`..`、不止一个最上层文件夹、空的包文件。
    BadPath(String),
    /// 解开超过了 [`MOST_ENTRIES`] 项或者 [`MOST_BYTES`] 字节。
    TooBig,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(error) => write!(f, "{error}"),
            Error::NotPlain(path) => write!(f, "not a plain file or folder: {path}"),
            Error::BadPath(path) => write!(f, "bad path in the package: {path}"),
            Error::TooBig => write!(f, "the package unpacks too big"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Error {
        Error::Io(error)
    }
}

/// 打好的一个包文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packed {
    /// 写在哪。
    pub path: PathBuf,
    /// 整个文件的 SHA-256，小写十六进制。
    pub sha256: String,
    /// 多少字节。
    pub size: u64,
}

/// 解开的一个包。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unpacked {
    /// 最上层文件夹的名字，就是包的编号。
    pub id: String,
    /// 解开的包文件夹。
    pub folder: PathBuf,
}

/// 包文件的名字：`<编号>-<版本>.miyu`，只能装在几种系统上的一种一个文件，名字后面带系统（`<编号>-<版本>-<系统>.miyu`）。
pub fn file_name(id: &str, version: &str, target: Option<Target>) -> String {
    match target {
        Some(target) => format!("{id}-{version}-{}.{EXTENSION}", target.as_str()),
        None => format!("{id}-{version}.{EXTENSION}"),
    }
}

/// 把包文件夹 `folder` 打成编号是 `id` 的包文件，写到 `out`（先写旁边的 `.partial`，成了再换过去）。
///
/// # Errors
///
/// 读写不成；文件夹里有链接、设备这些，名字不是 UTF-8 的。
pub fn pack(folder: &Path, id: &str, out: &Path) -> Result<Packed, Error> {
    let items = walk(folder)?;
    let partial = out.with_extension(format!("{EXTENSION}.partial"));
    let written = write(&items, id, &partial);
    let (sha256, size) = match written {
        Ok(done) => done,
        Err(error) => {
            discard(&partial);
            return Err(error);
        }
    };
    if let Err(error) = fs::rename(&partial, out) {
        discard(&partial);
        return Err(error.into());
    }
    Ok(Packed {
        path: out.to_path_buf(),
        sha256,
        size,
    })
}

/// 包文件夹里的一项。
struct Item {
    /// 相对包文件夹，用 `/` 分开。
    relative: String,
    /// 在磁盘上的位置。
    path: PathBuf,
    /// 是文件夹；不是的是文件。
    folder: bool,
    /// 能执行。
    executable: bool,
}

/// 文件夹里的每一项，照相对路径的字节排好；不打进去的（[`skipped`]）不要、也不往里走。
fn walk(folder: &Path) -> Result<Vec<Item>, Error> {
    let mut items = Vec::new();
    let mut stack = vec![(folder.to_path_buf(), String::new())];
    while let Some((dir, prefix)) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                return Err(Error::BadPath(format!(
                    "{prefix}{}",
                    name.to_string_lossy()
                )));
            };
            if skipped(name) {
                continue;
            }
            let relative = format!("{prefix}{name}");
            let meta = fs::symlink_metadata(entry.path())?;
            if meta.is_dir() {
                stack.push((entry.path(), format!("{relative}/")));
            } else if !meta.is_file() {
                return Err(Error::NotPlain(relative));
            }
            items.push(Item {
                executable: meta.is_file()
                    && (relative.starts_with("bin/") || executable_bit(&meta)),
                folder: meta.is_dir(),
                path: entry.path(),
                relative,
            });
        }
    }
    items.sort_by(|a, b| a.relative.cmp(&b.relative));
    Ok(items)
}

/// 名字是 `name` 的这一项不打进去（施工 G-13）：在 [`SKIPPED`] 里，或者以 [`SKIPPED_SUFFIX`] 结尾。
fn skipped(name: &str) -> bool {
    SKIPPED.contains(&name) || name.ends_with(SKIPPED_SUFFIX)
}

/// Unix 上有没有执行位；别的系统上没有。
#[cfg(unix)]
fn executable_bit(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable_bit(_meta: &fs::Metadata) -> bool {
    false
}

/// 照 `items` 写包文件 `to`，交回 SHA-256 和字节数。
fn write(items: &[Item], id: &str, to: &Path) -> Result<(String, u64), Error> {
    let hashing = Hashing {
        inner: File::create(to)?,
        hasher: Sha256::new(),
        size: 0,
    };
    let mut encoder = zstd::Encoder::new(hashing, LEVEL)?;
    encoder.include_checksum(true)?;
    let mut builder = tar::Builder::new(encoder);
    builder.append_data(&mut header(0, 0o755, true), format!("{id}/"), io::empty())?;
    for item in items {
        let mode = if item.folder || item.executable {
            0o755
        } else {
            0o644
        };
        if item.folder {
            let path = format!("{id}/{}/", item.relative);
            builder.append_data(&mut header(0, mode, true), path, io::empty())?;
        } else {
            let file = File::open(&item.path)?;
            let size = file.metadata()?.len();
            let path = format!("{id}/{}", item.relative);
            builder.append_data(&mut header(size, mode, false), path, file.take(size))?;
        }
    }
    let encoder = builder.into_inner()?;
    let mut hashing = encoder.finish()?;
    hashing.inner.flush()?;
    hashing.inner.sync_all()?;
    let digest = hashing.hasher.finalize();
    let sha256 = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok((sha256, hashing.size))
}

/// 一项的头：时间、属主都写 0，只留权限。
fn header(size: u64, mode: u32, folder: bool) -> tar::Header {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(if folder {
        tar::EntryType::Directory
    } else {
        tar::EntryType::Regular
    });
    header.set_size(size);
    header.set_mode(mode);
    header.set_mtime(0);
    header.set_uid(0);
    header.set_gid(0);
    header
}

/// 边写边算哈希、数字节。
struct Hashing {
    inner: File,
    hasher: Sha256,
    size: u64,
}

impl Write for Hashing {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buf)?;
        self.hasher.update(&buf[..written]);
        self.size += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// 删掉写了一半的；删不掉的留着。
#[expect(
    clippy::let_underscore_must_use,
    reason = "写了一半的删不掉就留着：后缀是 .partial，不会被当成包"
)]
fn discard(path: &Path) {
    let _ = fs::remove_file(path);
}

/// 把包文件 `file` 解进 `into`（没有的建上，有的要是空的）。
///
/// # Errors
///
/// 读写不成、不是 zstd 压的 tar；有链接、设备这些；路径不对；太大。出错时 `into` 里可能留着解了一半的，调用的人删。
pub fn unpack(file: &Path, into: &Path) -> Result<Unpacked, Error> {
    unpack_within(file, into, MOST_ENTRIES, MOST_BYTES)
}

/// 同 [`unpack`]，最多 `most_entries` 项、`most_bytes` 字节。
fn unpack_within(
    file: &Path,
    into: &Path,
    most_entries: usize,
    most_bytes: u64,
) -> Result<Unpacked, Error> {
    fs::create_dir_all(into)?;
    if fs::read_dir(into)?.next().is_some() {
        return Err(Error::Io(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "the folder to unpack into is not empty",
        )));
    }
    let decoder = zstd::Decoder::new(File::open(file)?)?;
    let mut archive = tar::Archive::new(decoder);
    let mut top: Option<String> = None;
    let (mut count, mut bytes) = (0_usize, 0_u64);
    for entry in archive.entries()? {
        let mut entry = entry?;
        count += 1;
        if count > most_entries {
            return Err(Error::TooBig);
        }
        let path = entry.path()?.into_owned();
        let (first, rest) = parts(&path)?;
        match &top {
            Some(top) if *top != first => {
                return Err(Error::BadPath(path.display().to_string()));
            }
            Some(_) => {}
            None => top = Some(first.clone()),
        }
        let target = into.join(&first).join(&rest);
        match entry.header().entry_type() {
            tar::EntryType::Directory => fs::create_dir_all(&target)?,
            tar::EntryType::Regular => {
                let size = entry.size();
                bytes = bytes.saturating_add(size);
                if bytes > most_bytes {
                    return Err(Error::TooBig);
                }
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut out = File::create_new(&target)?;
                let copied = io::copy(&mut (&mut entry).take(size), &mut out)?;
                if copied != size {
                    return Err(Error::Io(io::ErrorKind::UnexpectedEof.into()));
                }
                set_mode(&out, entry.header().mode()? & 0o111 != 0)?;
            }
            _ => return Err(Error::NotPlain(path.display().to_string())),
        }
    }
    let id = top.ok_or_else(|| Error::BadPath(String::new()))?;
    Ok(Unpacked {
        folder: into.join(&id),
        id,
    })
}

/// 一项的路径拆成最上层的名字和下面的部分；绝对路径、`..`、名字不是 UTF-8 的不行。
fn parts(path: &Path) -> Result<(String, PathBuf), Error> {
    let bad = || Error::BadPath(path.display().to_string());
    let mut names = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(name) => names.push(name.to_str().ok_or_else(bad)?.to_string()),
            _ => return Err(bad()),
        }
    }
    let mut names = names.into_iter();
    let first = names.next().ok_or_else(bad)?;
    Ok((first, names.collect()))
}

/// 解开的文件照包里写的给权限：能执行的 `755`，别的 `644`；Unix 以外不管。
#[cfg(unix)]
fn set_mode(file: &File, executable: bool) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = if executable { 0o755 } else { 0o644 };
    file.set_permissions(fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn set_mode(_file: &File, _executable: bool) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests;
