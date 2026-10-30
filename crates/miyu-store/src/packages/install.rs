//! 装、卸在磁盘上怎么做（施工 F-5 上，设计 `30-插件框架.md` 第九节，`docs/blueprint/packages.md`「装卸」）：只动数据根的
//! `packages/`。
//!
//! - 装：一个文件夹就是一个包（施工 F-8 上），整个文件夹拷成 `<编号>/`。先拷到点开头的暂存处，再换进去；原来就有的先挪到
//!   点开头的备份处，[`Placed::keep`] 删备份，[`Placed::undo`] 放回去。
//! - 卸家目录的：删掉整个文件夹。卸出厂的：记一笔 `<编号>.removed`（空文件），资源目录不动，删掉这一笔就回来。
//! - 换进去那一下改名失败的，当场把备份放回去（施工 AE-7）；两次改名之间核心被杀的，下一次起来时 [`recover`] 放回去。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// 卸掉的出厂的包记的那一笔的后缀。
pub const REMOVED: &str = ".removed";

/// 换进去了的一个包：留着原来那一份的备份，好撤回。
#[derive(Debug)]
pub struct Placed {
    /// 换进去的包文件夹。
    folder: PathBuf,
    /// 原来那一份挪到的地方；原来没有的没有。
    old: Option<PathBuf>,
}

impl Placed {
    /// 就这样了：删掉备份。删不掉的留着，不碍事（点开头的不算包）。
    #[expect(
        clippy::let_underscore_must_use,
        reason = "备份删不掉就留着：点开头的不算包，下一次装同一个包时先清掉"
    )]
    pub fn keep(self) {
        if let Some(old) = &self.old {
            let _ = fs::remove_dir_all(old);
        }
    }

    /// 撤回：删掉换进去的，原来的放回去。
    ///
    /// # Errors
    ///
    /// 删不掉、放不回去。
    pub fn undo(self) -> io::Result<()> {
        remove_dir_if_there(&self.folder)?;
        if let Some(old) = &self.old {
            fs::rename(old, &self.folder)?;
        }
        Ok(())
    }
}

/// 把包文件夹 `source` 装成家目录 `home` 里的包 `id`。
///
/// # Errors
///
/// 建不了目录、拷不了、换不进去；出了错的不留暂存。
pub fn place(home: &Path, id: &str, source: &Path) -> io::Result<Placed> {
    fs::create_dir_all(home)?;
    let staged = home.join(format!(".{id}.new"));
    remove_dir_if_there(&staged)?;
    if let Err(error) = copy_dir(source, &staged) {
        discard(&staged);
        return Err(error);
    }
    let folder = home.join(id);
    let old = aside(&folder, home.join(format!(".{id}.old")))?;
    if let Err(error) = fs::rename(&staged, &folder) {
        put_back(old.as_deref(), &folder);
        discard(&staged);
        return Err(error);
    }
    Ok(Placed { folder, old })
}

/// 核心起来时收拾家目录 `home` 里上一次装、升级到一半剩下的（施工 AE-7）：这时没有在装的，点开头、名字是
/// `.<编号>.old`、`.<编号>.new`（编号合写法）的都是剩下的。`<编号>/` 不在、`.<编号>.old` 在的（两次改名之间被杀、换进去失败
/// 又没放回去），放回去；别的删掉。别的名字一个都不碰。目录读不了的当没有。
pub fn recover(home: &Path) -> Recovered {
    let mut recovered = Recovered::default();
    let Ok(entries) = fs::read_dir(home) else {
        return recovered;
    };
    let mut leftovers: Vec<(String, &str, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let rest = name.strip_prefix('.')?;
            let (id, kind) = rest
                .strip_suffix(".old")
                .map(|id| (id, "old"))
                .or_else(|| rest.strip_suffix(".new").map(|id| (id, "new")))?;
            crate::personas::valid(id).then(|| (id.to_string(), kind, entry.path()))
        })
        .collect();
    // 先放回去，再删：同一个编号的 `.old`、`.new` 都在时，放回去的是原来那一份。
    leftovers.sort_by_key(|(_, kind, _)| *kind == "new");
    for (id, kind, path) in leftovers {
        let folder = home.join(&id);
        let done = match kind {
            "old" if fs::symlink_metadata(&folder).is_err() => {
                fs::rename(&path, &folder).map(|()| recovered.restored.push(id))
            }
            _ => remove_dir_if_there(&path),
        };
        if let Err(error) = done {
            recovered.failed.push((path, error));
        }
    }
    recovered
}

/// [`recover`] 收拾的结果。
#[derive(Debug, Default)]
pub struct Recovered {
    /// 放回去的包的编号。
    pub restored: Vec<String>,
    /// 放不回去、删不掉的：在哪，为什么。
    pub failed: Vec<(PathBuf, io::Error)>,
}

/// 换进去失败了，把挪开的原来那一份放回去：放不回去的留在备份处，下一次起来时 [`recover`] 再放。
#[expect(
    clippy::let_underscore_must_use,
    reason = "放不回去就留在备份处，起来时再放；报的是换进去那一下的错"
)]
fn put_back(old: Option<&Path>, folder: &Path) {
    if let Some(old) = old {
        let _ = fs::rename(old, folder);
    }
}

/// 删掉家目录 `home` 里的包 `id`：整个文件夹。
///
/// # Errors
///
/// 删不掉。
pub fn take_out(home: &Path, id: &str) -> io::Result<()> {
    remove_dir_if_there(&home.join(id))
}

/// 记一笔：出厂的包 `id` 卸掉了。
///
/// # Errors
///
/// 建不了目录、写不了。
pub fn mark_removed(home: &Path, id: &str) -> io::Result<()> {
    fs::create_dir_all(home)?;
    fs::write(home.join(format!("{id}{REMOVED}")), b"")
}

/// 删掉那一笔：出厂的包 `id` 装回来。
///
/// # Errors
///
/// 删不掉。
pub fn unmark_removed(home: &Path, id: &str) -> io::Result<()> {
    remove_if_there(&home.join(format!("{id}{REMOVED}")))
}

/// 家目录 `home` 里记着卸掉的出厂的包的编号。目录读不了的当没有。
pub fn removed(home: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(home) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let id = name.strip_suffix(REMOVED)?;
            crate::personas::valid(id).then(|| id.to_string())
        })
        .collect();
    ids.sort();
    ids
}

/// 拷到一半出错，删掉暂存：删不掉的留着（点开头的不算包，下一次装同一个包时先清掉），报原来那个错。
#[expect(
    clippy::let_underscore_must_use,
    reason = "删暂存是收拾，删不掉不改报的错"
)]
fn discard(staged: &Path) {
    let _ = remove_dir_if_there(staged);
}

/// 有的话挪到 `to`，交回挪到哪；没有的没有。
fn aside(path: &Path, to: PathBuf) -> io::Result<Option<PathBuf>> {
    if fs::symlink_metadata(path).is_err() {
        return Ok(None);
    }
    remove_if_there(&to)?;
    remove_dir_if_there(&to)?;
    fs::rename(path, &to)?;
    Ok(Some(to))
}

/// 一层层拷目录 `from` 到 `to`：文件照内容拷，链接照它指的拷。
fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if fs::metadata(entry.path())?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn remove_if_there(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

fn remove_dir_if_there(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}
