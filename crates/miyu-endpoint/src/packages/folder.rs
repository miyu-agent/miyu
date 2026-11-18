//! 从哪个包文件夹装（施工 F-5 上，G-1 从 `manage.rs` 挪出来：那一份到了 500 行）：装、看一眼共用。

use std::path::{Path, PathBuf};

use miyu_config::Words;
use miyu_config::package::{self, System, Target};
use miyu_store::human::Human;
use miyu_store::packages::{MANIFEST, Packages, RUNNING};

use super::manage::invalid;
use super::sentence;
use crate::Core;
use crate::config::methods::words;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 从 `path` 装之前查的（装、看一眼共用）：包文件夹（写成里面的 `package.toml` 的取它的上一层）、编号（文件夹名）、读成了的
/// 清单。路径不是绝对的、编号不合写法的 `bad_params`；读不了的 `path_unreadable`；写错的、编号是保留名字的 `package_invalid`；清单写了
/// `targets`、里面没有本机这一种的，写了 `systems`、里面没有本机的系统的 `wrong_target`（施工 G-1、PL-2）；和出厂的撞了的
/// `package_exists`。
pub(super) fn source(
    core: &Core,
    peer: Peer,
    path: &Path,
) -> Result<(PathBuf, String, package::Manifest), Refusal> {
    let (folder, id, manifest) = read(core, peer, path)?;
    let shipped = Packages::shipped(&core.resources).read();
    if shipped.iter().any(|one| one.id == id) {
        return Err(Refusal::PACKAGE_EXISTS);
    }
    Ok((folder, id, manifest))
}

/// 同 [`source`]，不和出厂的比（施工 U23 下上）。
pub(super) fn read(
    core: &Core,
    peer: Peer,
    path: &Path,
) -> Result<(PathBuf, String, package::Manifest), Refusal> {
    let folder = folder_of(path).ok_or(Refusal::BAD_PARAMS)?;
    let id = id_of(path).ok_or(Refusal::BAD_PARAMS)?;
    let text =
        std::fs::read_to_string(folder.join(MANIFEST)).map_err(|_| Refusal::PATH_UNREADABLE)?;
    let words = words(core, peer.language)?;
    // 保留的编号整份不收（设计 55 第三节第 3 条）：先于读清单。
    if let Some(problem) = miyu_store::packages::reserved_id(&id) {
        return Err(invalid(&words, &problem));
    }
    // 要更新的 Miyu 的照这一版报（施工 X-31）：同读装了的包，照 `miyu --version` 那一个版本号（施工 VER-1）。
    let manifest =
        package::read_for(&text, RUNNING).map_err(|problem| refused(&words, &text, &problem))?;
    if !package::fits_here(
        &manifest.systems,
        &manifest.targets,
        (System::here(), Target::here()),
    ) {
        return Err(Refusal::WRONG_TARGET);
    }
    Ok((folder.to_path_buf(), id, manifest))
}

/// 清单读不成：照 [`invalid`] 说；可能是这一版 Miyu 太旧的（不认识的表、键，清单又写了 `[package] miyu`，施工 VER-1）在后面补一句
/// 「这个包可能要更新的 Miyu」（`package-problems/maybe_newer_miyu`，字段 `problem`）。
fn refused(words: &Human, text: &str, problem: &package::Problem) -> Refusal {
    let said = sentence(words, problem.code.as_str(), &problem.detail)
        .unwrap_or_else(|| problem.message.clone());
    let said = match package::maybe_newer(text, problem) {
        true => Words::sentence(
            words,
            "package-problems/maybe_newer_miyu",
            &[("problem", &said)],
        )
        .unwrap_or(said),
        false => said,
    };
    Refusal::package_invalid(said, problem.line)
}

/// 包文件夹：写成里面的 `package.toml` 的取它的上一层。
fn folder_of(path: &Path) -> Option<&Path> {
    match path.file_name().and_then(|name| name.to_str()) {
        Some(MANIFEST) => path.parent(),
        _ => Some(path),
    }
}

/// 从 `path` 装的编号：包文件夹的名字，合写法、是绝对路径的才有（施工 IN-3：核对之前就推进度，照它说是哪个包）。
pub(super) fn id_of(path: &Path) -> Option<String> {
    let folder = folder_of(path)?;
    folder
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|id| miyu_store::personas::valid(id))
        .map(str::to_string)
        .filter(|_| folder.is_absolute())
}
