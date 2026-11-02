//! 两个暂存（设计 `55-数据根仿FHS.md` 第三节第 5 条）：`tmp/` 短命，像 `/tmp`，核心起来只清自己的 `tmp/unpack/`；`var/tmp/`
//! 跨重启留着，像 `/var/tmp`，核心起来时清掉放了满 [`STALE_VAR_TMP`] 没碰的；包的 `tmp/<编号>/` 拉起它之前清。
//!
//! 清之前从数据根往下逐层看是真目录（[`DataRoot::plain_down`]）：`tmp/`、`var/`、`var/tmp/` 哪一层是链接，就会顺着它删到数据根
//! 外面去（施工 FH-3下，core1 审 FH-3上 时提的），整趟不清、交回错。

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
        self.clear(&self.unpack_staging())
    }

    /// 拉起包 `id` 之前清掉它上一回留下的短命暂存 `tmp/<编号>/`（设计 55 第三节第 5 条）。
    ///
    /// # Errors
    ///
    /// 路上哪一层是链接、不是目录；删不掉。
    pub fn clear_package_tmp(&self, id: &str) -> io::Result<()> {
        self.clear(&self.package_tmp(id))
    }

    /// 整个删掉数据根下面的目录 `dir`：路上每一层是真目录才删，本来没有的不算错。
    fn clear(&self, dir: &Path) -> io::Result<()> {
        self.plain_down(dir)?;
        match fs::remove_dir_all(dir) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }

    /// 从数据根往下到 `dir`（含它自己）的每一层：在的都得是真目录，链接、文件都报错；不在的那一层往下不看，下面也没有东西。
    ///
    /// # Errors
    ///
    /// 哪一层是链接、不是目录（说是哪一层）；`dir` 不在数据根下面；读不了。
    fn plain_down(&self, dir: &Path) -> io::Result<()> {
        let below = dir
            .strip_prefix(&self.path)
            .map_err(|_| io::Error::other(format!("{} is outside the data root", dir.display())))?;
        let mut at = self.path.clone();
        for part in below.components() {
            at.push(part);
            match fs::symlink_metadata(&at) {
                Ok(metadata) if metadata.is_dir() => {}
                Ok(_) => {
                    return Err(io::Error::other(format!(
                        "{} is a link or not a directory: not clearing through it",
                        at.display()
                    )));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    /// 清 `var/tmp/` 下面放了满 [`STALE_VAR_TMP`] 没碰的：一项一项看，目录看它整棵里最近改过的那一刻，不跟链接。
    ///
    /// # Errors
    ///
    /// `var/`、`var/tmp/` 是链接、不是目录（整趟不清）；`var/tmp/` 读不了（没有这个目录的不算错）。
    pub fn sweep_var_tmp(&self, now: SystemTime) -> io::Result<Swept> {
        let mut swept = Swept::default();
        self.plain_down(&self.var_tmp())?;
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
