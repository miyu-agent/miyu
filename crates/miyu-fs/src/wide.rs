//! 工作目录太宽（`11-权限与沙盒.md` 第四节「当前目录太宽」，施工 4-3 下）：根目录、落在数据根里的，不拿它当工作区。原来系统的
//! 家目录、包含数据根的也算；施工 5-14 改起不算（[`too_wide`]），数据根、私密目录靠沙盒藏住（Windows 施工 SW-7 跟上），加进来的
//! 目录、场所会话照旧（[`broad`]）。
//!
//! 施工 5-14 起工作目录和工作区分开（11 第四节「工作目录和工作区分开」）：太宽的照样是工作目录（命令从这里起），能随便写的
//! 工作区退回这个会话的默认工作区（[`workspace_of`]）。

use std::path::{Path, PathBuf};

/// 真实的位置 `dir` 太不太宽，不能拿它当工作区（施工 5-14 改）：根目录，落在数据根 `data_root` 里而不在数据根的工作区 `own`
/// 里的。家目录本身、包住数据根的不算（2026-10-22 项目主人定）：Linux 有 bubblewrap 的挂载盖住、只有 Landlock 的照 dsh，
/// macOS 照 Seatbelt 的拒绝规则，Windows 照 MXC 的禁止压过读写（施工 SW-7，2026-10-31 项目主人定，虚拟机上量过）。几样都要
/// 先换成真实的位置再交进来；`home` 不看，留着和 [`broad`] 一个样子。
pub fn too_wide(dir: &Path, _home: Option<&Path>, data_root: &Path, own: &Path) -> bool {
    dir.parent().is_none() || (dir.starts_with(data_root) && !dir.starts_with(own))
}

/// 宽（施工 4-3 下）：是家目录 `home`、是根目录，包含数据根 `data_root`，或者落在数据根里而不在数据根的工作区 `own` 里。
/// 加进来的目录、场所会话的工作目录照它（施工 5-14 改：这两样照旧）。几样都要先换成真实的位置再交进来。
pub fn broad(dir: &Path, home: Option<&Path>, data_root: &Path, own: &Path) -> bool {
    home == Some(dir)
        || dir.parent().is_none()
        || data_root.starts_with(dir)
        || (dir.starts_with(data_root) && !dir.starts_with(own))
}

/// 工作目录 `cwd` 对应的工作区（施工 5-14）：太宽的（[`too_wide`]）是这个会话的默认工作区 `fallback`，别的就是工作目录本身。
/// 几样都要先换成真实的位置再交进来；`fallback` 还没建的照写，用到它的一方自己建。
pub fn workspace_of(
    cwd: &Path,
    home: Option<&Path>,
    data_root: &Path,
    own: &Path,
    fallback: &Path,
) -> PathBuf {
    match too_wide(cwd, home, data_root, own) {
        true => fallback.to_path_buf(),
        false => cwd.to_path_buf(),
    }
}
