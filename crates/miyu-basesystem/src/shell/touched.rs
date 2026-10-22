//! 前台命令改了她看过的文件（施工 E-D 补，项目主人 2026-10-23 定「认前台 shell 改的」）：命令起来之前记下她看过的每个文件的
//! 修改时刻和大小（不读内容），跑完再看一遍，变了的重读、算哈希，和她看过的不一样的交一个 `file.read` 效果：执行器照它把
//! 「看过的版本」换成新的，之后 `edit`、`write` 当是她自己改的，不要她重读。
//!
//! 只认前台跑完的（到时被杀的也算）：挪到后台的、被叫停的不认。代价：命令跑着时外面恰好改了同一个文件，也当成她改的、
//! 不再拦。

use std::path::PathBuf;
use std::time::SystemTime;

use miyu_kernel::id::ContentHash;
use miyu_tool::{Effect, Seen};

/// 命令起来之前她看过的那些文件：路径、她看过的哈希、那时的修改时刻和大小（不在的、读不了的是空的）。
pub(super) struct Before(Vec<(PathBuf, ContentHash, Option<Stamp>)>);

/// 修改时刻和大小。
type Stamp = (SystemTime, u64);

/// 记下 `seen` 里每个文件这一刻的修改时刻和大小。
pub(super) fn before(seen: &Seen) -> Before {
    Before(
        seen.iter()
            .map(|(path, hash)| (path.clone(), hash.clone(), stamp(path)))
            .collect(),
    )
}

/// 跑完了：修改时刻、大小变了的重读，内容和她看过的不一样的交一个 `file.read`（整份，不带行）。不在了、不是文件、读不了的
/// 不交：照旧，`edit` 照它们原来的规矩报。
pub(super) fn after(before: Before) -> Vec<Effect> {
    before
        .0
        .into_iter()
        .filter(|(path, _, then)| stamp(path).is_some_and(|now| Some(now) != *then))
        .filter_map(|(path, seen, _)| {
            let hash = ContentHash::of(&std::fs::read(&path).ok()?);
            (hash != seen).then_some(Effect::Read {
                path,
                lines: None,
                hash,
            })
        })
        .collect()
}

/// 一个普通文件的修改时刻和大小；不在的、不是文件的是空的。
fn stamp(path: &PathBuf) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    meta.is_file()
        .then_some((meta.modified().ok()?, meta.len()))
}
