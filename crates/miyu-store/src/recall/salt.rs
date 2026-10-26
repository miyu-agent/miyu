//! 运行日志里怎么写一间记忆（施工 LG-1 下（记忆），`docs/blueprint/log.md` 第七节）：不写账号、人格名、会话编号，写种类
//! （[`Room::kind`]）和一个短号（[`LogSalt::room_id`]）。短号照数据根自己的盐算：同一台机器上人格名就那么几个，不加盐的短哈希
//! 一个个试就对得回去；盐只在这个数据根的 `state/log-salt` 里，不进日志、不进任何回应，日志拿到别的机器上对不回去。

use std::fmt;
use std::fs;
use std::io::{self, Write as _};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use super::Room;
use crate::durable;
use crate::root::DataRoot;

/// 盐在状态区里的文件名。
const FILE: &str = "log-salt";
/// 盐几个字节。
const LEN: usize = 32;
/// 短号取摘要的前几个字节（写成两倍长的十六进制）。
const SHORT: usize = 4;

/// 一个数据根的盐。`Debug` 不印它。
#[derive(Clone, PartialEq, Eq)]
pub struct LogSalt([u8; LEN]);

impl fmt::Debug for LogSalt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LogSalt(..)")
    }
}

impl LogSalt {
    /// 数据根 `root` 的盐：`state/log-salt` 有、长度对的读它；没有的（或者长度不对、坏了的）新造一份写进去。两个进程同时
    /// 第一次造的，后到的读先写下的那份。
    ///
    /// # Errors
    ///
    /// 读不了、建不了状态区、写不进、系统给不出随机字节：交给调的一方记一行，这次起来用 [`LogSalt::fresh`]。
    pub fn load(root: &DataRoot) -> io::Result<LogSalt> {
        let state = root.state();
        let path = state.join(FILE);
        if let Some(salt) = read(&path)? {
            return Ok(salt);
        }
        let mut bytes = [0u8; LEN];
        getrandom::fill(&mut bytes).map_err(io::Error::other)?;
        durable::create_dir(&state)?;
        let (temp, mut file) =
            durable::create_temp_with(&state, || durable::temp_name(FILE), Some(0o600))?;
        let written = file.write_all(&bytes).and_then(|()| file.sync_all());
        drop(file);
        if let Err(error) = written {
            durable::discard(&temp);
            return Err(error);
        }
        // 只许新建：有了（别的进程刚写下的，或者长度不对的旧文件）就不盖。长度不对的旧文件先挪开，再放新的。
        if path.exists() && read(&path)?.is_none() {
            fs::remove_file(&path)?;
        }
        let linked = fs::hard_link(&temp, &path);
        durable::discard(&temp);
        match linked {
            Ok(()) => {
                durable::sync_dir(&state)?;
                Ok(LogSalt(bytes))
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                read(&path)?.ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))
            }
            Err(error) => Err(error),
        }
    }

    /// 只在这个进程里的盐：读不出、写不成 `state/log-salt` 时用。系统给不出随机字节的，照此刻和进程号凑一份（只是这一次
    /// 起来的日志对不上下一次的，不挡启动）。
    pub fn fresh() -> LogSalt {
        let mut bytes = [0u8; LEN];
        if getrandom::fill(&mut bytes).is_err() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos());
            let mut hasher = Sha256::new();
            hasher.update(now.to_le_bytes());
            hasher.update(std::process::id().to_le_bytes());
            bytes.copy_from_slice(&hasher.finalize());
        }
        LogSalt(bytes)
    }

    /// 一间记忆在运行日志里的短号：盐接上这一间的种类、账号、编号，SHA-256 取前 4 个字节，8 位小写十六进制。同一个盐、
    /// 同一间每次一样。
    pub fn room_id(&self, room: &Room) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.0);
        hasher.update(room.to_string().as_bytes());
        let digest = hasher.finalize();
        digest
            .iter()
            .take(SHORT)
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

/// 读 `path` 的盐：没有这个文件的、长度不对的是 `None`。
fn read(path: &Path) -> io::Result<Option<LogSalt>> {
    match fs::read(path) {
        Ok(bytes) => Ok(<[u8; LEN]>::try_from(bytes.as_slice()).ok().map(LogSalt)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;
