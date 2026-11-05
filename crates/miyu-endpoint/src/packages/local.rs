//! 本地库接到装卸上（施工 F-8 中上，设计 `31-软件包.md` 第四节，`miyu_store::packages::local`）：家目录里装成、升级成的记一份
//! （每个文件的路径、SHA-256、字节数），卸掉的删掉那一份；核心起来时给以前装的、还没记的补上（照现在的文件算）。
//! `package.info`、`package.files` 照它答；出厂的不进本地库，照现在的文件现算。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_store::packages::local::{self, Desc, Entry, Kind};
use miyu_store::packages::{Found, Layer, Packages};

use super::approve::Came;
use super::{TARGET, packages};
use crate::Core;
use crate::refusal::Refusal;

/// 记下家目录里的包 `id`：照包目录现在的文件算，`source` 是从哪装的（补的是空的）；从软件源装的另记来路 `came`（来源仓库，施工
/// G-4 上；软件源的名字，施工 U23 下下，官方的不记）。施工 PK-3 上起来路随装一起记，不再装完另补：装之前要照旧的判断来路变没变。
/// 施工 PK-6 起记是怎么来的（`kind`）：有来路的照来路，从本机路径装的是 `file`；补的（`source` 是空的）不知道，不记，留给
/// [`infer`]。
///
/// # Errors
///
/// 包目录读不了、本地库写不进。
pub(super) fn record(
    places: &Packages,
    id: &str,
    version: Option<String>,
    (source, came): (&str, Option<&Came>),
) -> std::io::Result<()> {
    let home = places
        .home_dir()
        .ok_or_else(|| std::io::Error::other("no home layer"))?;
    let files = local::scan(&home.join(id))?;
    let desc = Desc {
        id: id.to_string(),
        version,
        installed: crate::sessions::now().to_string(),
        source: source.to_string(),
        size: local::size(&files),
        origin: came.map(|came| came.origin.clone()),
        feed: came.and_then(|came| came.feed.clone()),
        kind: match (came, source.is_empty()) {
            (Some(came), _) => Some(came.kind),
            (None, false) => Some(Kind::File),
            (None, true) => None,
        },
    };
    local::write(&places.local_root(), &desc, &files)
}

/// 核心起来时：本地库里没记是怎么来的包（施工 PK-6 以前装的），照能推出来的推一次、写回 `desc`（[`local::rewrite_desc`]，整份
/// 暂存再换上），各记一行 `INFO package kind inferred`；推不出来的不写，下次起来再推，协议上说 `unknown`：
///
/// - 有来源仓库、有软件源的名字：配置里自己加的源，一律算社区；
/// - 有来源仓库、没有软件源的名字（官方源装的）：照本机存的官方索引（`official` 目录，照 `keys` 验签）里这个编号、来源仓库对得上的
///   那一条，官方收录的算官方，不然算社区；索引没有、验签不过、编号不在、来源仓库对不上的不知道；
/// - 没有来源仓库、记了从哪个路径装的：本机文件；
/// - 都没有（核心起来时补记的）：不知道。
///
/// 官方索引最多读一次、验一次签名（core1 2026-11-05：算进起来那段的耗时），一个都不用推的不读。
pub(crate) fn infer(places: &Packages, official: &Path, keys: &[miyu_source::sign::PublicKey]) {
    let root = places.local_root();
    let mut index: Option<Option<miyu_source::index::Index>> = None;
    for id in local::ids(&root) {
        let Some(mut desc) = local::desc(&root, &id) else {
            continue;
        };
        if desc.kind.is_some() {
            continue;
        }
        let kind = match (&desc.origin, &desc.feed) {
            (Some(_), Some(_)) => Some(Kind::Community),
            (Some(origin), None) => index
                .get_or_insert_with(|| super::source::sync::stored_index(official, keys))
                .as_ref()
                .and_then(|index| {
                    index
                        .packages
                        .iter()
                        .find(|entry| entry.id == id && &entry.origin == origin)
                })
                .map(|entry| match entry.official {
                    true => Kind::Official,
                    false => Kind::Community,
                }),
            (None, _) if !desc.source.is_empty() => Some(Kind::File),
            (None, _) => None,
        };
        let Some(kind) = kind else {
            continue;
        };
        desc.kind = Some(kind);
        match local::rewrite_desc(&root, &desc) {
            Ok(()) => {
                tracing::info!(target: TARGET, package = id.as_str(), kind = kind.as_str(), "package kind inferred");
            }
            Err(error) => {
                tracing::warn!(target: TARGET, package = id.as_str(), error = %error, "package kind not written");
            }
        }
    }
}

/// 核心起来时：家目录里读成了、还没记的包补一份，各记一行 `INFO package recorded`；补不成的 `WARN package not recorded`。
pub(crate) fn backfill(places: &Packages, found: &[Found]) {
    let root = places.local_root();
    let known = local::ids(&root);
    for one in found {
        let Ok(manifest) = &one.read else {
            continue;
        };
        if one.layer != Layer::Home || known.contains(&one.id) {
            continue;
        }
        match record(places, &one.id, manifest.version.clone(), ("", None)) {
            Ok(()) => tracing::info!(target: TARGET, package = one.id.as_str(), "package recorded"),
            Err(error) => {
                tracing::warn!(target: TARGET, package = one.id.as_str(), error = %error, "package not recorded");
            }
        }
    }
}

/// `package.files` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FilesParams {
    package: String,
}

/// 一个包这时的样子：哪一层、包目录、清单写的版本，和它的记录。
pub(super) struct Looked {
    pub(super) layer: Layer,
    pub(super) dir: PathBuf,
    pub(super) version: Option<String>,
    pub(super) desc: Option<Desc>,
    pub(super) files: Vec<Entry>,
}

/// 一个包这时的记录：装进来的照本地库（没记的现算），出厂的现算。
pub(super) async fn entry(core: &Arc<Core>, id: &str) -> Result<Looked, Refusal> {
    let now = core.system_packages();
    let found: &Found = now
        .iter()
        .find(|one| one.id == id)
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    let version = found
        .read
        .as_ref()
        .ok()
        .and_then(|manifest| manifest.version.clone());
    let places = packages(core);
    let (folder, root, layer) = (found.files_dir(), places.local_root(), found.layer);
    let target = id.to_string();
    let dir = folder.clone();
    let read = tokio::task::spawn_blocking(move || {
        // 本地库只记装进数据根的（出厂的、系统加装的不记，施工 SD-1）。
        let recorded = (layer == Layer::Home)
            .then(|| local::read(&root, &target))
            .flatten();
        match recorded {
            Some((desc, files)) => Ok((Some(desc), files)),
            None => local::scan(&folder).map(|files| (None, files)),
        }
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?;
    let (desc, files) = read.map_err(|error| {
        tracing::warn!(target: TARGET, package = id, error = %error, "package files unreadable");
        Refusal::INTERNAL
    })?;
    Ok(Looked {
        layer,
        dir,
        version,
        desc,
        files,
    })
}

/// 装着的包 `id` 的基本几格（`package.info` 装着的那一份从它起，施工 IN-2 在 [`super::info`] 另加几格；她的软件包工具只用这几格）：
/// 编号、哪一层、版本、从哪来（`from`、`source`、`feed`，施工 PK-6）、装的时刻和从哪个路径装的（`path`，本地库里记了的才有）、几个
/// 文件、多少字节。不替主人干活的（`for_owner` 是假的）不给 `path`：那是这台电脑上的路径。
pub(crate) async fn info(core: &Arc<Core>, for_owner: bool, id: &str) -> Result<Value, Refusal> {
    let Looked {
        layer,
        version,
        desc,
        files,
        ..
    } = entry(core, id).await?;
    let mut answer = json!({
        "package": id,
        "layer": layer.as_str(),
        "files": files.len(),
        "size": local::size(&files),
    });
    let version = desc
        .as_ref()
        .and_then(|desc| desc.version.clone())
        .or(version);
    if let Some(version) = version {
        answer["version"] = json!(version);
    }
    // 作者、上游、从哪来（施工 F-12；`source`、`feed` 施工 PK-6）：同 `package.list` 的那几格。
    let now = core.system_packages();
    if let Some(found) = now.iter().find(|one| one.id == id) {
        let places = packages(core);
        super::fill::origin(&mut answer, found, &places);
        if let Ok(manifest) = &found.read {
            for (key, value) in [
                ("author", &manifest.author),
                ("upstream", &manifest.upstream),
            ] {
                if let Some(value) = value {
                    answer[key] = json!(value);
                }
            }
        }
    }
    if let Some(desc) = desc {
        answer["installed"] = json!(desc.installed);
        // 从哪个路径装的，只给主人看（施工 PK-6 起叫 `path`，同 `package.info {path}` 回应里那一格；`source` 改说是怎么来的）。
        if for_owner && !desc.source.is_empty() {
            answer["path"] = json!(desc.source);
        }
    }
    Ok(answer)
}

/// `package.files {package}`：包目录 `dir`（绝对路径，施工 F-8 下：命令行照 pacman 印绝对路径），每个文件的 `path`（相对
/// 包目录，`/` 分开）、`sha256`、`size`，照路径排。
pub(crate) async fn files(core: &Arc<Core>, params: FilesParams) -> Result<Value, Refusal> {
    let looked = entry(core, &params.package).await?;
    let files: Vec<Value> = looked
        .files
        .iter()
        .map(|file| json!({"path": file.path, "sha256": file.sha256, "size": file.size}))
        .collect();
    Ok(json!({ "dir": looked.dir, "files": files }))
}

/// 本地库里删掉包 `id`（卸掉以后）；删不掉的记一行，不算卸失败。
pub(super) fn forget(root: &Path, id: &str) {
    if let Err(error) = local::remove(root, id) {
        tracing::warn!(target: TARGET, package = id, error = %error, "package record not removed");
    }
}

#[cfg(test)]
mod tests;
