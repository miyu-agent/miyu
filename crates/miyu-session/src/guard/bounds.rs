//! 照一次调用的工作目录、加进来的目录拼边界表（从 `guard.rs` 挪来，施工 X-37 补：报路径的工具判路径、管理员放行的判文件在不在
//! 她写得了的地方，两处共用一份）。

use std::path::{Path, PathBuf};
use std::sync::PoisonError;

use miyu_fs::{Boundary, Places, resolve};

use super::Guard;

/// 一次调用的边界。
pub(super) struct Bounds {
    /// 工作目录的真实位置（头报来的可能是 `~`）。
    pub(super) cwd: PathBuf,
    /// 能随便写的工作区：太宽的工作目录退回这个会话的默认工作区（施工 5-14）。
    pub(super) workspace: PathBuf,
    /// 拼边界表用的几个地方。
    pub(super) places: Places,
    /// 边界表。
    pub(super) boundary: Boundary,
}

impl Guard {
    /// 工作目录 `cwd`、加进来的目录 `dirs`（施工 5-10 上，照工作目录的办法换成真实的位置）这一次的边界。
    pub(super) fn bounds(&self, cwd: &str, dirs: &[String]) -> Bounds {
        let home = self.home.as_deref();
        let real = |path: &str| {
            resolve(Path::new(path), home, path).unwrap_or_else(|_| PathBuf::from(path))
        };
        let cwd = real(cwd);
        let workspace = self.workspace(&cwd);
        let places = Places {
            workspace: workspace.clone(),
            dirs: dirs.iter().map(|dir| real(dir)).collect(),
            shelves: self
                .shelves
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .iter()
                .chain(&self.attachments)
                .cloned()
                .collect(),
            private: self
                .private
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .clone(),
            readonly: self
                .readonly
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .clone(),
            ..self.places.clone()
        };
        let boundary = Boundary::new(&places);
        Bounds {
            cwd,
            workspace,
            places,
            boundary,
        }
    }
}
