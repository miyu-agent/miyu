//! 一次调用带的沙盒（`internal/blueprint/session/tools.md` 第 1a 条，施工 5-4 上）：照派出去那一刻实际生效的那一级写
//! 规格。完全放开不进沙盒（A16 起本人的会话只剩完全放开、只读两级，施工 LV-1）；只读哪儿都不能写，什么都不藏（施工 RO-2，
//! 2026-11-16 项目主人定：只读就是完全放开减去写，数据根放开读；别的程序的私密目录施工 PV-RM 起就不藏）；关起来的会话不看
//! 级别，另照 `confined.rs`。

mod confined;

use std::io;
use std::path::{Path, PathBuf};

use miyu_kernel::event::Permission;
use miyu_sandbox::{Sandboxed, Spec};

use crate::default_workspace::DefaultWorkspace;
use crate::guard::{Effective, effective};

/// 给调用写沙盒要的：助手在哪、系统的家目录、数据根。一个会话一份，这台机器上的沙盒能用才有。
#[derive(Debug, Clone)]
pub(crate) struct Sandbox {
    helper: PathBuf,
    home: Option<PathBuf>,
    data_root: PathBuf,
    /// 关起来的会话（施工 5-12 下）：写只限工作区，读另加数据根、藏着凭据那几处（`confined.rs`，施工 RD-1）。
    confined: bool,
    /// 开着的技能文件夹（施工 X-10 中）：关起来的会话加进只准读的名单。
    shelves: Vec<PathBuf>,
    /// 工作目录太宽时退回哪（施工 5-14）：关起来的会话能写的就是它。
    workplace: Option<DefaultWorkspace>,
}

impl Sandbox {
    /// 助手是 `helper`，`~` 照 `home` 换，数据根是 `data_root`（关起来的会话藏它）；关起来的会话关到 `confined`（施工 5-12 下）。
    pub(crate) fn new(
        helper: PathBuf,
        home: Option<PathBuf>,
        data_root: PathBuf,
        confined: bool,
    ) -> Sandbox {
        Sandbox {
            helper,
            home,
            data_root,
            confined,
            shelves: Vec::new(),
            workplace: None,
        }
    }

    /// 工作目录太宽时退回的这个会话的默认工作区（施工 5-14）。
    #[must_use]
    pub(crate) fn with_workplace(mut self, workplace: DefaultWorkspace) -> Sandbox {
        self.workplace = Some(workplace);
        self
    }

    /// 开着的技能文件夹（施工 X-10 中，`skills.md`「技能」第 3 条）：外部身份的会话加进只准读的名单。
    #[must_use]
    pub(crate) fn with_shelves(mut self, shelves: Vec<PathBuf>) -> Sandbox {
        self.set_shelves(shelves);
        self
    }

    /// 换上这一刻开着的技能文件夹（施工 X-11 下）：回合开始照它换。
    pub(crate) fn set_shelves(&mut self, shelves: Vec<PathBuf>) {
        self.shelves = shelves;
    }

    /// 照实际生效的那一级 `permission`、这一轮的工作目录 `cwd` 和加进来的目录 `dirs`（施工 5-10 上）写一次调用的沙盒：
    /// 完全放开的不带。关起来的会话碰磁盘（换真实的位置、建目录），在阻塞线程里调。
    ///
    /// # Errors
    ///
    /// 关起来的会话工作区里的 `.tmp` 建不成。
    pub(crate) fn for_call(
        &self,
        permission: &Permission,
        cwd: &str,
        dirs: &[String],
    ) -> io::Result<Option<Sandboxed>> {
        // 关起来的会话不看级别，读写都只限工作区（施工 5-12 下）。
        if self.confined {
            return self.confined_call(permission, (cwd, dirs)).map(Some);
        }
        if effective(permission) == Effective::Full {
            return Ok(None);
        }
        // A16 起本人的会话只在只读时进沙盒（施工 LV-1）：整盘只读，哪儿都不能写，什么都不藏（施工 RO-2）。
        Ok(Some(Sandboxed {
            helper: self.helper.clone(),
            spec: Spec {
                write: Vec::new(),
                hidden: Vec::new(),
                read: None,
                reveal: Vec::new(),
                // 脚本调工具的通道由 `shell` 起命令时加（施工 M-3）。
                sockets: Vec::new(),
            },
            env: Vec::new(),
            uncached: false,
        }))
    }
}

/// 换成真实的位置；换不成的照原样，助手会再换一次，换不成就不跑。
fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests;
