//! 会话在哪干活（从 `sessions.rs` 挪出来，那边放不下了）：工作目录照人走，太宽的标上（能写的工作区由会话退回它的默认工作区，
//! 施工 5-14），加进来的目录太宽的整条命令都不收（`11-权限与沙盒.md` 第四节，施工 4-3 下、5-10 上）。

use std::path::{Path, PathBuf};

use miyu_kernel::id::SessionId;

use super::Opening;

use crate::Core;
use crate::refusal::Refusal;

/// 加进来的目录里有太宽的：整条命令都不收（施工 5-10 上）。
pub(crate) fn check_dirs(core: &Core, dirs: &[String]) -> Result<(), Refusal> {
    if dirs.iter().any(|dir| dir_too_wide(core, dir)) {
        Err(Refusal::DIR_TOO_WIDE)
    } else {
        Ok(())
    }
}

/// 加进来的一个目录太不太宽：`~` 本身、系统的家目录、根目录、包含数据根的（`miyu_fs::broad`：施工 5-14 改起工作目录不再
/// 这样算，加进来的目录照旧，2026-10-22 主会话定），另外落在数据根里的一律算太宽，`workspace/` 也不例外：工作目录太宽时有地方
/// 可退，加进来的目录没有。换不成真实位置的照原样，边界表里那一片不算。
fn dir_too_wide(core: &Core, dir: &str) -> bool {
    if dir.trim() == "~" {
        return true;
    }
    let home = core
        .home
        .as_deref()
        .and_then(|home| std::fs::canonicalize(home).ok());
    let Ok(real) = miyu_fs::resolve(Path::new("/"), home.as_deref(), dir) else {
        return false;
    };
    let data_root =
        std::fs::canonicalize(core.root.path()).unwrap_or_else(|_| core.root.path().to_path_buf());
    miyu_fs::within(&real, &data_root)
        || miyu_fs::broad(&real, home.as_deref(), &data_root, &data_root)
}

/// 挑好的工作目录：实际用的写法，和它是不是太宽（施工 9-7 补；施工 5-14 起太宽的照样是工作目录，能写的工作区退回这个会话
/// 的默认工作区，会话那边算）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Picked {
    /// 实际用的目录：照原样的写法，退回的是这个会话的默认工作区。
    pub(crate) cwd: String,
    /// 太宽（根目录；Windows 上还有 `~` 本身、系统的家目录、包含数据根的，施工 5-14 改）：照用，工作区是会话的默认工作区。
    pub(crate) wide: bool,
    /// 宽（`miyu_fs::broad`：`~` 本身、系统的家目录、根目录、包含数据根的）：场所会话照它退回默认工作区（施工 5-14 改照旧）。
    pub(crate) broad: bool,
}

/// 照 `cwd` 挑工作目录（`11-权限与沙盒.md` 第四节「工作目录和工作区分开」，施工 5-14）：人在哪就是哪，太宽的照用、标上
/// `wide`（2026-10-20 项目主人定，原来头自己带上的太宽照旧退回、人明着选的才照用）。落在数据根里、又不在
/// `workspace/` 里的，`~` 本身读不出家目录的，退回这个会话的默认工作区 `fallback`（建好）。换不成真实位置的照原样：
/// 说不清它宽不宽，用到时工具自己报错。
pub(crate) fn pick(core: &Core, cwd: &str, fallback: &Path) -> Picked {
    let fallback = || prepared(core, fallback);
    let home = crate::workspace::home(core);
    let Ok(real) = miyu_fs::resolve(Path::new("/"), home.as_deref(), cwd) else {
        // `~` 本身读不出家目录的：说不清在哪，退回。
        return match cwd.trim() == "~" {
            true => fallback(),
            false => Picked {
                cwd: cwd.to_string(),
                wide: false,
                broad: false,
            },
        };
    };
    let (data_root, own) = roots(core);
    if miyu_fs::within(&real, &data_root) && !miyu_fs::within(&real, &own) {
        return fallback();
    }
    Picked {
        cwd: cwd.to_string(),
        wide: miyu_fs::too_wide(&real, home.as_deref(), &data_root, &own),
        broad: miyu_fs::broad(&real, home.as_deref(), &data_root, &own),
    }
}

/// 真实位置 `real` 的工作目录太不太宽（施工 9-7 补；`workspace.rs` 查过人明着要换去的以后照它说 `wide`）。
pub(crate) fn wide(core: &Core, real: &Path) -> bool {
    let (data_root, own) = roots(core);
    miyu_fs::too_wide(
        real,
        crate::workspace::home(core).as_deref(),
        &data_root,
        &own,
    )
}

/// 退回这个会话的默认工作区 `dir`：建好（建数据根时就建了 `workspace/`，会话的那一个用到才建；建不了的照样退回，用到时工具
/// 自己报错），交回它。
fn prepared(core: &Core, dir: &Path) -> Picked {
    if let Err(error) = core
        .root
        .prepare_workspace()
        .and_then(|()| std::fs::create_dir_all(dir))
    {
        tracing::warn!(target: "miyu::endpoint", kind = ?error.kind(), "workspace not prepared");
    }
    Picked {
        cwd: dir.to_string_lossy().into_owned(),
        wide: false,
        broad: false,
    }
}

/// 造会话 `id` 时的工作目录（施工 5-14）：照 [`pick`] 挑；`workspace/` 本身（网页不选目录时交的就是它，握手的
/// `host.workspace`）换成这个会话的默认工作区；场所会话宽的（没带目录的是 `~`，照 [`Picked::broad`]：施工 5-14 改照旧）也用它：
/// 外部身份只碰得到自己的工作区。
/// 交回工作目录和太不太宽。只在造会话时这样认：以前的会话记着的照旧。
pub(crate) fn opening_cwd(core: &Core, who: &Opening, cwd: &str, id: &SessionId) -> (String, bool) {
    let fallback = core.root.session_workspace(id);
    let picked = match account_workspace(core, cwd) {
        true => prepared(core, &fallback),
        false => pick(core, cwd, &fallback),
    };
    match picked.broad && who.venue.is_some() {
        true => (prepared(core, &fallback).cwd, false),
        false => (picked.cwd, picked.wide),
    }
}

/// 人明着要换去 `workspace/` 本身（网页的「默认工作区」交的就是它，握手的 `host.workspace`）：换成这个会话自己的默认工作区
/// `default`，建好（施工 5-14 中）。别的交回空。
pub(crate) fn to_default(core: &Core, cwd: &str, default: &Path) -> Option<Picked> {
    account_workspace(core, cwd).then(|| prepared(core, default))
}

/// 工作目录是不是 `workspace/` 本身。
fn account_workspace(core: &Core, cwd: &str) -> bool {
    let home = crate::workspace::home(core);
    let (_, own) = roots(core);
    miyu_fs::resolve(Path::new("/"), home.as_deref(), cwd).is_ok_and(|real| real == own)
}

/// 数据根、`workspace/`，都换成真实的位置：和人交来的路径同一个换法（`miyu_fs::resolve`：还不存在的照最近的、已经存在的
/// 上级目录算），两边才比得上（施工 5-14 中修：macOS 的 `/var` 是链接、Windows 有短名，原来不存在的照原样，对不上）。
/// 换不成的照原样。
pub(crate) fn roots(core: &Core) -> (PathBuf, PathBuf) {
    let real = |path: PathBuf| {
        miyu_fs::resolve(Path::new("/"), None, &path.to_string_lossy()).unwrap_or(path)
    };
    (
        real(core.root.path().to_path_buf()),
        real(core.root.workspace()),
    )
}
