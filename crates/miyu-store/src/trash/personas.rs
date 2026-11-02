//! 删掉的人格（施工 P-3 下，`docs/blueprint/personas.md`「改」）：家目录里的人格目录整个挪进
//! `trash/personas/<编号>.<删的时刻，毫秒>/`，里面多一个 `deleted_at`（同会话的写法）；核心起来时和会话一起清，
//! 删了满时限的真删（同会话，7 天）。同一个编号删几次各是各的。挪是一次改名：要么挪了、要么没挪。
//!
//! 删完三层里都没有这个编号了的，它的记忆日志 `root/memory/<编号>/` 跟着挪进同一处的 `memory/`（设计 55 第三节第 6 条，
//! [`take_memory`]），派生的记忆库删掉；清回收处时一起删。

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

use miyu_kernel::time::Timestamp;

use super::{DELETED_AT, deleted_at};
use crate::durable::{create_dir, sync_dir};
use crate::root::DataRoot;

/// 把人格目录 `from`（编号 `id`）挪进回收处，删的时刻是 `at`：先在目录里写下 `deleted_at`、同步，再改名，
/// 同步两头的上一层。回收处没有的先建。
///
/// # Errors
///
/// 目录不在；写不了 `deleted_at`；建不了回收处；改不了名；同步不了。
pub fn discard(root: &DataRoot, from: &Path, id: &str, at: Timestamp) -> io::Result<()> {
    let trash = root.trashed_personas();
    let mut stamp = File::create(from.join(DELETED_AT))?;
    stamp.write_all(format!("{at}\n").as_bytes())?;
    stamp.sync_all()?;
    drop(stamp);
    create_dir(&trash)?;
    fs::rename(from, trash.join(format!("{id}.{}", at.unix_millis())))?;
    if let Some(personas) = from.parent() {
        sync_dir(personas)?;
    }
    sync_dir(&trash)
}

/// 记忆在回收处那一份人格目录里的名字。
pub const MEMORY: &str = "memory";

/// 人格 `id`（删的时刻是 `at`，[`discard`] 刚挪进回收处的那一份）的记忆日志跟着挪进去：`root/memory/<编号>/` 改名成
/// `trash/personas/<编号>.<毫秒>/memory/`，派生的记忆库（`var/cache/index/recall/memory-<编号>.db`）删掉，同一个编号以后
/// 再有记忆时照日志重建。没有记忆日志的什么都不做，交回假。记忆库删不掉的不算错（以后照日志补）。
///
/// # Errors
///
/// 回收处里没有那一份；改不了名；同步不了。
pub fn take_memory(root: &DataRoot, id: &str, at: Timestamp) -> io::Result<bool> {
    let from = root.memory_log(id);
    if !from.is_dir() {
        return Ok(false);
    }
    let entry = root
        .trashed_personas()
        .join(format!("{id}.{}", at.unix_millis()));
    fs::rename(&from, entry.join(MEMORY))?;
    let room = crate::recall::Room::Persona {
        persona: id.to_string(),
    };
    remove_derived(&room.memories(root));
    sync_dir(&root.memory_logs())?;
    sync_dir(&entry)?;
    Ok(true)
}

/// 删掉派生的库文件 `path`：连同 SQLite 旁边的 `-wal`、`-shm`。删不掉的留着。
#[expect(clippy::let_underscore_must_use, reason = "派生的，删不掉以后照日志补")]
fn remove_derived(path: &Path) {
    let _ = fs::remove_file(path);
    for suffix in ["-wal", "-shm"] {
        let mut side = path.as_os_str().to_os_string();
        side.push(suffix);
        let _ = fs::remove_file(Path::new(&side));
    }
}

/// 清了一次人格的回收处。
#[derive(Debug, Default)]
pub struct Purged {
    /// 真删掉了几个。
    pub removed: usize,
    /// 留下了、却不是因为没满时限的：读不出删的时刻的，删不掉的。哪一个（回收处里的名字），为什么（系统的原话，英文）。
    pub failed: Vec<(String, String)>,
}

/// 清一次人格回收处：删的时刻离 `now` 满了 `keep` 的连目录整个删掉；没满的、时钟往回拨过的留着；读不出
/// 删的时刻的留着，记进 [`Purged::failed`]。回收处还没有的什么都不做。
///
/// # Errors
///
/// 回收处读不了。
pub fn purge(root: &DataRoot, now: Timestamp, keep: Duration) -> io::Result<Purged> {
    let mut purged = Purged::default();
    let entries = match fs::read_dir(root.trashed_personas()) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(purged),
        Err(error) => return Err(error),
    };
    let keep = i64::try_from(keep.as_millis()).unwrap_or(i64::MAX);
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let deleted = match deleted_at(&dir) {
            Ok(deleted) => deleted,
            Err(why) => {
                purged.failed.push((name, why));
                continue;
            }
        };
        if now.unix_millis().saturating_sub(deleted.unix_millis()) < keep {
            continue;
        }
        match fs::remove_dir_all(&dir) {
            Ok(()) => purged.removed += 1,
            Err(error) => purged.failed.push((name, error.to_string())),
        }
    }
    Ok(purged)
}

#[cfg(test)]
mod tests;
