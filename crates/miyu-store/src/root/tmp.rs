//! 两个暂存（设计 `55-数据根仿FHS.md` 第三节第 5 条）：`tmp/` 短命，像 `/tmp`，核心起来只清自己的 `tmp/unpack/`；`var/tmp/`
//! 跨重启留着，像 `/var/tmp`，核心起来时清掉放了满 [`STALE_VAR_TMP`] 没碰的。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::{DataRoot, TMP, VAR};

/// `var/tmp/` 里一项多久没碰就清（设计 55 第三节第 5 条：初值 7 天）。
pub const STALE_VAR_TMP: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// 清 `var/tmp/` 清了什么、哪些没清掉。
#[derive(Debug, Default)]
pub struct Swept {
    /// 清掉的。
    pub removed: Vec<PathBuf>,
    /// 清不掉的，和原因。
    pub failed: Vec<(PathBuf, io::Error)>,
}

impl DataRoot {
    /// 跨重启留着的暂存：`var/tmp/`，按时限清（[`DataRoot::sweep_var_tmp`]）。
    pub fn var_tmp(&self) -> PathBuf {
        self.path.join(VAR).join(TMP)
    }

    /// 核心起来时清的短命暂存：核心自己的 `tmp/unpack/`。包的 `tmp/<编号>/` 在拉起那个包之前清。
    ///
    /// # Errors
    ///
    /// 删不掉。
    pub fn clear_own_tmp(&self) -> io::Result<()> {
        match fs::remove_dir_all(self.unpack_staging()) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }

    /// 清 `var/tmp/` 下面放了满 [`STALE_VAR_TMP`] 没碰的：一项一项看，目录看它整棵里最近改过的那一刻，不跟链接。
    ///
    /// # Errors
    ///
    /// `var/tmp/` 读不了（没有这个目录的不算错）。
    pub fn sweep_var_tmp(&self, now: SystemTime) -> io::Result<Swept> {
        let mut swept = Swept::default();
        let entries = match fs::read_dir(self.var_tmp()) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(swept),
            Err(error) => return Err(error),
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let fresh = newest(&path).is_none_or(|touched| {
                now.duration_since(touched)
                    .is_ok_and(|age| age < STALE_VAR_TMP)
            });
            if fresh {
                continue;
            }
            match remove(&path) {
                Ok(()) => swept.removed.push(path),
                Err(error) => swept.failed.push((path, error)),
            }
        }
        Ok(swept)
    }
}

/// `path` 整棵里最近改过的那一刻（不跟链接）；什么都读不了的是 `None`，当刚碰过，不清。
fn newest(path: &Path) -> Option<SystemTime> {
    let metadata = fs::symlink_metadata(path).ok()?;
    let mut newest = metadata.modified().ok()?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path).ok()?.flatten() {
            let inner = newest_or_now(&entry.path());
            newest = newest.max(inner);
        }
    }
    Some(newest)
}

/// 同 [`newest`]，读不了的当刚碰过：一棵里有一处读不了，整棵都不清。
fn newest_or_now(path: &Path) -> SystemTime {
    newest(path).unwrap_or_else(SystemTime::now)
}

/// 删掉 `path`：目录整棵删，别的（连同链接本身）只删它自己。
fn remove(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

#[cfg(test)]
mod tests;
