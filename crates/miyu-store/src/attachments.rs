//! 附件落成会话里的文件（施工 3-9 六补，`docs/blueprint/store.md`「会话目录」）：剪贴板贴的、截图、远程的头传内容的附件
//! 没有本机路径，核心把 blob 拷成会话目录 `attachments/` 下的一个文件，她照这个路径读、带给别的会话。放在会话目录里：
//! 删会话时跟着挪进回收处，恢复时跟着回来，不另写清理（项目主人 2026-10-16 定）。拷，不硬链接：blob 按内容存、不许改。

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use miyu_kernel::id::{AccountId, SessionId};

use crate::durable::{create_dir, create_temp, sync_dir, temp_name};
use crate::root::DataRoot;

/// 文件名最多几个字节（鲁棒性 B-8：原来照字符数算 120 个，120 个汉字 360 字节，超过常见文件系统一个名字 255 字节的上限）。
/// 留出撞名的 ` (1000)`、临时名的 `.<名字>.<进程号>-<计数>.tmp`，还在 255 以内；Windows 照 UTF-16 算，只会更短。
const NAME_BYTES: usize = 180;

/// 扩展名（带点）最多几个字节：截名字时留着它，没有扩展名时照媒体类型补的也只补这么长的。常见的都在这以内。
const EXTENSION_BYTES: usize = 16;

/// 名字清干净以后什么都不剩时用的名字。
const FALLBACK: &str = "attachment";

/// 撞名时最多试到第几个序号。
const TRIES: u32 = 1000;

/// Windows 上不能当文件名的几个（不分大小写，带不带扩展名都不行）。
const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// 会话 `session` 的附件目录：`home/<账号>/sessions/<会话>/attachments/`。
pub fn dir(root: &DataRoot, account: &AccountId, session: &SessionId) -> PathBuf {
    root.session_dir(account, session).join("attachments")
}

/// 把 `bytes` 存进附件目录 `dir`，交回文件的真实位置。名字照 `name`（[`clean`]），没有扩展名的照媒体类型 `media_type`
/// 斜杠后面那一段补上；同名的文件内容一样的就用它（同一张图贴了两次），不一样的接 ` (2)`、` (3)`。先写临时文件、
/// 落盘以后再改名，别处看不到写了一半的文件；临时文件的名字照进程号、计数起（鲁棒性 B-7：原来名字是定的，崩溃留下一个，
/// 这个名字的附件就再也存不进）。
///
/// # Errors
///
/// 建不了目录、写不了、改不了名、同步不了；一连 1000 个名字都被占了。
pub fn keep(dir: &Path, name: &str, media_type: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    create_dir(dir)?;
    let name = with_extension(&clean(name), media_type);
    let (stem, extension) = split(&name);
    for n in 1..=TRIES {
        let candidate = match n {
            1 => name.clone(),
            n => format!("{stem} ({n}){extension}"),
        };
        let path = dir.join(&candidate);
        match fs::read(&path) {
            Ok(existing) if existing == bytes => return fs::canonicalize(&path),
            Ok(_) => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let (temp, mut file) = create_temp(dir, || temp_name(&candidate))?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        if path.exists() {
            fs::remove_file(&temp)?;
            continue;
        }
        fs::rename(&temp, &path)?;
        sync_dir(dir)?;
        return fs::canonicalize(&path);
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("{TRIES} names in a row are taken in {}", dir.display()),
    ))
}

/// 头交的名字清成能放进任何平台的文件名：路径分隔符、控制字符、Windows 不认的 `:*?"<>|` 换成 `_`；去掉两头的空格和点
/// （Windows 会悄悄去掉，名字就对不上了）；Windows 的保留名前面加 `_`；截到 180 个字节以内（截在字的边界上，扩展名
/// 留着）；什么都不剩的叫 `attachment`。
pub fn clean(name: &str) -> String {
    let replaced: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = replaced.trim_matches([' ', '.']);
    let clipped = clip(trimmed);
    let clipped = clipped.trim_end_matches([' ', '.']);
    if clipped.is_empty() {
        return FALLBACK.to_string();
    }
    let base = clipped.split('.').next().unwrap_or_default();
    if RESERVED
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(base))
    {
        return format!("_{clipped}");
    }
    clipped.to_string()
}

/// 截到 [`NAME_BYTES`] 个字节以内，截在字的边界上：有扩展名（不长过 [`EXTENSION_BYTES`]）的截前面那一段、扩展名留着，
/// 别的整个截。
fn clip(name: &str) -> String {
    if name.len() <= NAME_BYTES {
        return name.to_string();
    }
    let (stem, extension) = match split(name) {
        (stem, extension) if extension.len() <= EXTENSION_BYTES => (stem, extension),
        _ => (name, ""),
    };
    let mut end = NAME_BYTES - extension.len();
    while !stem.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{extension}", &stem[..end])
}

/// 没有扩展名的接上媒体类型斜杠后面那一段（`image/png` → `.png`），那一段只认字母、数字，最长 [`EXTENSION_BYTES`]（带点）；
/// 认不出的照原样。
fn with_extension(name: &str, media_type: &str) -> String {
    if !split(name).1.is_empty() {
        return name.to_string();
    }
    let subtype = media_type.split('/').nth(1).unwrap_or_default();
    if subtype.is_empty()
        || subtype.len() >= EXTENSION_BYTES
        || !subtype.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return name.to_string();
    }
    format!("{name}.{subtype}")
}

/// 拆成名字和扩展名（扩展名带点）；开头的点不算扩展名。
fn split(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(at) if at > 0 => (&name[..at], &name[at..]),
        _ => (name, ""),
    }
}

#[cfg(test)]
mod tests;
