//! 装包文件（施工 G-1，设计 `37-软件源.md` 第二节第 5 条）：`package.install` 的 `path` 是一个 `.miyu` 文件的，先解进数据根里
//! 点开头的暂存处 `state/packages/.unpack/<一次一个>/`，再照包文件夹装（看一眼也一样）；本地库记的来源是那个文件。暂存处
//! 用完就删。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use miyu_source::archive::{self, EXTENSION};

use super::{TARGET, packages};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;
use crate::reverse::Peer as Reverse;

/// `path` 是不是一个包文件：后缀是 `.miyu`。
pub(super) fn is_package_file(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()) == Some(EXTENSION)
}

/// 装包文件 `path`（`preview` 是只看一眼）。路径不是绝对的 `bad_params`；读不了的 `path_unreadable`；解不开
/// 的 `package_file_bad`；之后同装包文件夹（进度推给 `caller`，从软件源装的自己推、不传）。
///
/// 写了 `expected` 的（从软件源装的，索引里那一条的编号，施工 AS-1）：包文件里的编号（最上层文件夹的名字）对不上也是
/// `package_file_bad`，什么都不装。不查的话，源里写着 `foo` 的条目装进来的可以是别的源装的 `bar`，盖掉它、接着用它的开关和
/// 批过的能力（设计 `37-软件源.md`「换源就是换信任的对象」）。
pub(super) async fn install(
    core: &Arc<Core>,
    peer: Peer,
    path: &Path,
    preview: bool,
    expected: Option<&str>,
    caller: Option<&Reverse>,
) -> Result<Value, Refusal> {
    if !path.is_absolute() {
        return Err(Refusal::BAD_PARAMS);
    }
    if !path.is_file() {
        return Err(Refusal::PATH_UNREADABLE);
    }
    let into = staging(&packages(core).state_root().join(".unpack"));
    let (file, place) = (path.to_path_buf(), into.clone());
    let unpacked = tokio::task::spawn_blocking(move || archive::unpack(&file, &place))
        .await
        .map_err(|_| Refusal::INTERNAL);
    let answer = match unpacked {
        Ok(Ok(unpacked)) if expected.is_some_and(|expected| expected != unpacked.id) => {
            tracing::warn!(target: TARGET, expected = expected.unwrap_or_default(), got = unpacked.id.as_str(), "package file holds another package");
            Err(Refusal::PACKAGE_FILE_BAD)
        }
        Ok(Ok(unpacked)) if preview => {
            super::preview::installing(core, peer, &unpacked.folder).await
        }
        Ok(Ok(unpacked)) => {
            super::manage::from_path(core, (peer, caller), &unpacked.folder, Some(path)).await
        }
        Ok(Err(error)) => {
            tracing::info!(target: TARGET, error = %error, "package file not unpacked");
            Err(Refusal::PACKAGE_FILE_BAD)
        }
        Err(refusal) => Err(refusal),
    };
    if let Err(error) = tokio::task::spawn_blocking(move || std::fs::remove_dir_all(&into))
        .await
        .unwrap_or_else(|_| Err(std::io::ErrorKind::Other.into()))
    {
        tracing::warn!(target: TARGET, error = %error, "unpacked package not cleaned up");
    }
    answer
}

/// 这一次解进哪：进程号加纳秒，同一个核心一次只装一个（`core.packaging` 锁着）。
fn staging(root: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    root.join(format!("{}-{nanos}", std::process::id()))
}
