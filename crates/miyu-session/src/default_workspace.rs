//! 工作目录对应的工作区（施工 5-14，`docs/designs/11-权限与沙盒.md` 第四节「工作目录和工作区分开」）：工作目录照人走，命令
//! 从那里起；能随便写的工作区平常就是它，太宽的（系统的家目录、根目录、包含数据根的）退回这个会话的默认工作区。权限策略、
//! 沙盒照同一个办法算（`miyu_fs::workspace_of`）。

use std::path::{Path, PathBuf};

use miyu_kernel::id::SessionId;
use miyu_store::root::DataRoot;

/// 一个会话算工作区要的：数据根的工作区 `workspace/`（落在它里面的不算太宽）、这个会话的默认工作区。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DefaultWorkspace {
    own: PathBuf,
    fallback: PathBuf,
}

impl DefaultWorkspace {
    /// 默认工作区用会话 `from` 的（主会话是自己，子会话、分叉出来的照 `session.created` 的
    /// `workspace_from`）。
    pub(crate) fn new(root: &DataRoot, from: &SessionId) -> DefaultWorkspace {
        DefaultWorkspace {
            own: root.workspace(),
            fallback: root.session_workspace(from),
        }
    }

    /// 真实位置是 `cwd` 的工作目录对应的工作区：家目录 `home`、数据根 `data_root` 都已换成真实的位置。默认工作区还没建的
    /// 照写（沙盒用到它时建）。
    pub(crate) fn of(&self, cwd: &Path, home: Option<&Path>, data_root: &Path) -> PathBuf {
        miyu_fs::workspace_of(
            cwd,
            home,
            data_root,
            &real(&self.own),
            &real(&self.fallback),
        )
    }

    /// 同 [`DefaultWorkspace::of`]，退回默认工作区的先建好、交回它真实的位置：边界表、Landlock 都不认不存在的路径，不建的话
    /// 文件工具写它会被当成数据根里的拒掉，沙盒收不住。
    ///
    /// # Errors
    ///
    /// 默认工作区建不成。
    pub(crate) fn made(
        &self,
        cwd: &Path,
        home: Option<&Path>,
        data_root: &Path,
    ) -> std::io::Result<PathBuf> {
        let workspace = self.of(cwd, home, data_root);
        if workspace.as_path() == cwd {
            return Ok(workspace);
        }
        std::fs::create_dir_all(&self.fallback)?;
        Ok(real(&self.fallback))
    }
}

impl DefaultWorkspace {
    /// 这个会话的默认工作区，没换成真实的位置（施工 5-14 中：头照它认「默认工作区」）。
    pub(crate) fn dir(&self) -> PathBuf {
        self.fallback.clone()
    }
}

/// 换成真实的位置；换不成的（还没建）照原样。
fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests;
