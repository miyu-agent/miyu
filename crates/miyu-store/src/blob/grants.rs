//! 包自己存过哪些 blob，落在盘上的那一份（施工 BG-1，`docs/blueprint/protocol.md` 的 `blob.get` 第 1 条）：扩展不带 `session` 只读
//! 得到它拿得到的 blob，其中「它自己存的」要过得了核心重起：表情包这样的包把图只存在 blob 里、自己只记哈希，重起以后手里没有
//! 内容，没法再存一次。经 `package.call` 转来的、别人交来的不记在这里：那是一次调用里临时给的。
//!
//! 一个包一份：`var/lib/blob-grants/<包的编号>`（[`DataRoot::blob_grants`]，设计 55 第三节：核心自己的状态；不放在包自己的状态
//! 目录里，那里包自己写得进，等于自己给自己开门）。纯文本，一行一个哈希（`sha256:` 加 64 位十六进制），只追加：同
//! `journal.jsonl`，每追加一行都重新打开、追加、同步，新建的文件另同步目录；一个核心里照一把锁一行一行来。存没存过由调的
//! 一方记着（端点的 `attach/grants.rs`），这里不查重。上一次写到一半断了电、最后没有换行的，下一次追加先补一个换行：那半行
//! 读的时候当坏行跳过，不和新的一行粘在一起。

use std::collections::{BTreeSet, HashMap};
use std::fs::{self, OpenOptions};
use std::io::{self, Read as _, Seek as _, SeekFrom, Write as _};
use std::sync::{Mutex, PoisonError};

use miyu_kernel::id::ContentHash;

use crate::durable::{create_dir, sync_dir};
use crate::root::DataRoot;

#[cfg(test)]
mod tests;

/// 追加照它一行一行来：一个核心里几个连接同时存的共用。
static WRITING: Mutex<()> = Mutex::new(());

/// 核心起来时读回来的。
#[derive(Debug, Default)]
pub struct Loaded {
    /// 包的编号到它自己存过的哈希。
    pub held: HashMap<String, BTreeSet<ContentHash>>,
    /// 有读不懂的行的那几份：包的编号、跳过了几行。
    pub bad: Vec<(String, usize)>,
    /// 整份读不了的那几份：包的编号、系统的原话。
    pub unreadable: Vec<(String, io::Error)>,
}

/// 读数据根 `root` 里记下的每一份。目录没有的是空的；名字不合包编号写法的不认（核心只写包的编号）；读不懂的行（不是一个
/// 哈希、不是 UTF-8、写到一半的）跳过，记在 [`Loaded::bad`]；一份读不了的不拦别的几份，记在 [`Loaded::unreadable`]。
///
/// # Errors
///
/// 目录在、却列不出来。
pub fn load(root: &DataRoot) -> io::Result<Loaded> {
    let mut loaded = Loaded::default();
    let entries = match fs::read_dir(root.blob_grants_dir()) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(loaded),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let Some(package) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !crate::layers::valid(&package) {
            continue;
        }
        let bytes = match fs::read(entry.path()) {
            Ok(bytes) => bytes,
            Err(error) => {
                loaded.unreadable.push((package, error));
                continue;
            }
        };
        let mut held = BTreeSet::new();
        let mut bad = 0;
        for line in bytes.split(|&byte| byte == b'\n') {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if line.is_empty() {
                continue;
            }
            match std::str::from_utf8(line).map(ContentHash::parse) {
                Ok(Ok(hash)) => {
                    held.insert(hash);
                }
                _ => bad += 1,
            }
        }
        if bad > 0 {
            loaded.bad.push((package.clone(), bad));
        }
        loaded.held.insert(package, held);
    }
    Ok(loaded)
}

/// 记下包 `package` 自己存了 `hash`：在它那一份末尾追加一行，落了盘才交回。目录、文件没有的建上。
///
/// # Errors
///
/// 数据根不在了、建不了、写不进、同步不了。
pub fn append(root: &DataRoot, package: &str, hash: &ContentHash) -> io::Result<()> {
    let _writing = WRITING.lock().unwrap_or_else(PoisonError::into_inner);
    let dir = root.blob_grants_dir();
    create_dir(&dir)?;
    let path = root.blob_grants(package);
    let existed = path.exists();
    let mut file = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(&path)?;
    if !existed {
        sync_dir(&dir)?;
    }
    let mut line = String::with_capacity(hash.as_str().len() + 2);
    if ends_mid_line(&mut file)? {
        line.push('\n');
    }
    line.push_str(hash.as_str());
    line.push('\n');
    file.write_all(line.as_bytes())?;
    file.sync_data()
}

/// 这份文件不空、最后一个字节不是换行：上一次写到一半断了。
fn ends_mid_line(file: &mut fs::File) -> io::Result<bool> {
    if file.metadata()?.len() == 0 {
        return Ok(false);
    }
    file.seek(SeekFrom::End(-1))?;
    let mut last = [0u8; 1];
    file.read_exact(&mut last)?;
    Ok(last[0] != b'\n')
}

/// 包 `package` 卸了：删掉它那一份。本来就没有的不算错。
///
/// # Errors
///
/// 在、却删不掉。
pub fn forget(root: &DataRoot, package: &str) -> io::Result<()> {
    let _writing = WRITING.lock().unwrap_or_else(PoisonError::into_inner);
    match fs::remove_file(root.blob_grants(package)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}
