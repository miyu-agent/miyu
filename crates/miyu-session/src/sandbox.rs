//! 一次调用带的沙盒（`docs/blueprint/session/tools.md` 第 1a 条，施工 5-4 上）：照派出去那一刻实际生效的那一级写
//! 规格。完全放开不进沙盒（A16 起本人的会话只剩完全放开、只读两级，施工 LV-1）；只读哪儿都不能写，藏数据根和别的程序的
//! 私密目录（施工 5-13）；关起来的会话不看级别，另照 `confined.rs`。

mod confined;

use std::io;
use std::path::{Path, PathBuf};

use miyu_fs::{private_dirs, within};
use miyu_kernel::event::Permission;
use miyu_sandbox::{Sandboxed, Spec};

use crate::default_workspace::DefaultWorkspace;
use crate::guard::{Effective, effective};

/// 给调用写沙盒要的：助手在哪、系统的家目录、数据根、沙盒的缓存。一个会话一份，这台机器上的沙盒能用才有。
#[derive(Debug, Clone)]
pub(crate) struct Sandbox {
    helper: PathBuf,
    home: Option<PathBuf>,
    data_root: PathBuf,
    /// 关起来的会话（施工 5-12 下）：读写都只限工作区（`confined.rs`）。
    confined: bool,
    /// 开着的技能文件夹（施工 X-10 中）：数据根藏着，它们单独放开只读。
    shelves: Vec<PathBuf>,
    /// 这个会话的附件目录（施工 3-9 六补）：同技能文件夹单独放开只读；还没建（没存过附件）的不放：Landlock 认不得
    /// 不在的路径。
    attachments: Option<PathBuf>,
    /// 这个会话放后台命令输出的目录 `jobs/`（施工 R3-1 补）：每条命令起来以前建好（`keep_places`）。
    jobs: Option<PathBuf>,
    /// 工作目录太宽时退回哪（施工 5-14）：没有的能写的就是工作目录。
    workplace: Option<DefaultWorkspace>,
    /// 这一轮的私密目录（施工 5-15 下）：回合开始照配置换；还没换过的照出厂的。
    private: Option<Vec<PathBuf>>,
}

impl Sandbox {
    /// 助手是 `helper`，`~` 照 `home` 换，藏的是 `data_root`；关起来的会话关到 `confined`（施工 5-12 下）。
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
            attachments: None,
            jobs: None,
            workplace: None,
            private: None,
        }
    }

    /// 工作目录太宽时退回的这个会话的默认工作区（施工 5-14）。
    #[must_use]
    pub(crate) fn with_workplace(mut self, workplace: DefaultWorkspace) -> Sandbox {
        self.workplace = Some(workplace);
        self
    }

    /// 这个会话的附件目录（施工 3-9 六补）。
    #[must_use]
    pub(crate) fn with_attachments(mut self, dir: PathBuf) -> Sandbox {
        self.attachments = Some(dir);
        self
    }

    /// 这个会话放后台命令输出的目录（施工 R3-1 补）。
    #[must_use]
    pub(crate) fn with_jobs(mut self, dir: PathBuf) -> Sandbox {
        self.jobs = Some(dir);
        self
    }

    /// 开着的技能文件夹（施工 X-10 中，`skills.md`「技能」第 3 条）：规格里单独放开只读（外部身份的会话加进只准读的名单）。
    #[must_use]
    pub(crate) fn with_shelves(mut self, shelves: Vec<PathBuf>) -> Sandbox {
        self.set_shelves(shelves);
        self
    }

    /// 换上这一刻开着的技能文件夹（施工 X-11 下）：回合开始照它换。
    pub(crate) fn set_shelves(&mut self, shelves: Vec<PathBuf>) {
        self.shelves = shelves;
    }

    /// 换上这一轮的私密目录（施工 5-15 下）：回合开始照配置换。
    pub(crate) fn set_private(&mut self, private: Vec<PathBuf>) {
        self.private = Some(private);
    }

    /// 照实际生效的那一级 `permission`、这一轮的工作目录 `cwd` 和加进来的目录 `dirs`（施工 5-10 上）写一次调用的沙盒：
    /// 完全放开的不带。碰磁盘（换真实的位置、建目录），在阻塞线程里调。
    ///
    /// # Errors
    ///
    /// 数据根的 `run/`、这个会话的 `jobs/` 建不成。
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
        let level = effective(permission);
        if level == Effective::Full {
            return Ok(None);
        }
        let data_root = real(&self.data_root);
        self.keep_places(&data_root)?;
        // A16 起本人的会话只在只读时进沙盒（施工 LV-1）：整盘只读，哪儿都不能写。
        let write = Vec::new();
        let hidden = self.hidden(data_root, &write);
        Ok(Some(Sandboxed {
            helper: self.helper.clone(),
            spec: Spec {
                write,
                hidden,
                read: None,
                reveal: self
                    .shelves
                    .iter()
                    .chain(self.attachments.iter().filter(|dir| dir.is_dir()))
                    .map(|dir| real(dir))
                    .collect(),
                // 脚本调工具的通道由 `shell` 起命令时加（施工 M-3）。
                sockets: Vec::new(),
            },
            env: Vec::new(),
            uncached: false,
        }))
    }
}

impl Sandbox {
    /// 藏起来的：数据根，加这台机器上有的别的程序的私密目录（施工 5-13，11 第四节；这一轮的配置加的、去掉的照它，施工 5-15 下）。落在能写的 `write` 里面的
    /// 只在藏得住时（[`hides_inside`]：macOS，施工 5-14 改）藏，不然不藏：私密目录照人给的算（同权限策略的
    /// 先后），数据根照 dsh 不挖洞（施工 5-14 改，原来 Landlock 一级级绕开、冻住工作区那一层）。
    fn hidden(&self, data_root: PathBuf, write: &[PathBuf]) -> Vec<PathBuf> {
        let mut hidden = Vec::new();
        // 数据根落在能写的里面（工作目录是 `~`，施工 5-14 改）：藏得住的照藏；只有 Landlock 的照 dsh 不挖洞（开会话时不提示，
        // 2026-10-22 项目主人定）；能写的以外照旧藏。Windows 照旧藏（MXC 的禁止压过读写）。
        if hides_inside() || cfg!(windows) || !write.iter().any(|base| within(&data_root, base)) {
            hidden.push(data_root);
        }
        let private = match &self.private {
            Some(private) => private.clone(),
            None => self.home.as_deref().map(private_dirs).unwrap_or_default(),
        };
        for dir in private
            .iter()
            .filter_map(|dir| std::fs::canonicalize(dir).ok())
        {
            // 人给的能写的落在它里面（或者就是它）的不藏：照人给的算，最深的那一片算（施工 5-14 改，同权限策略的边界表）。
            let given = write.iter().any(|base| within(base, &dir));
            if !given && (hides_inside() || !write.iter().any(|base| within(&dir, base))) {
                outermost(&mut hidden, dir);
            }
        }
        hidden
    }
}

/// 能写的范围里的藏不藏得住（施工 5-14 改）：macOS 的 Seatbelt 深的拒绝规则压得过浅的允许，藏得住（工作目录可以是 `~` 以后，
/// `~` 里的私密目录就落在能写的里面）。Landlock 挖不掉（施工 5-16 用 bubblewrap 的挂载盖过，施工 LV-5 去掉了）。
fn hides_inside() -> bool {
    cfg!(target_os = "macos")
}

/// 藏起来的加上 `dir`，彼此不重叠（施工 5-15 下修）：落在已有的一条里面的（一样的也算）不加，已有的落在它里面的去掉：
/// 藏了外层也就藏了里面。
fn outermost(hidden: &mut Vec<PathBuf>, dir: PathBuf) {
    if hidden.iter().any(|outer| within(&dir, outer)) {
        return;
    }
    hidden.retain(|inner| !within(inner, &dir));
    hidden.push(dir);
}

impl Sandbox {
    /// 命令起来以前要在的几处，不在的建上（施工 R3-1、R3-1 补；FH-3）：数据根里核心起来时建的那几处（`etc/private/` 凭据、
    /// `run/` 写配置、密钥的临时文件，`DataRoot::prepare_private`），这个会话的 `jobs/`（后台命令的输出）。Windows 的沙盒只禁
    /// 命令起来那一刻已有的（施工 SW-6），命令跑着时才建的目录里面读得到（2026-11-02 虚拟机上量过）：核心跑着时被人删掉的，
    /// 每条命令前补回来。数据根没有标记的（不是数据根）不建。
    fn keep_places(&self, data_root: &Path) -> io::Result<()> {
        if std::fs::symlink_metadata(data_root.join(miyu_store::root::MARKER)).is_err() {
            return Ok(());
        }
        miyu_store::root::DataRoot::at(data_root.to_path_buf()).prepare_private()?;
        match &self.jobs {
            Some(jobs) => miyu_store::root::create_inside(jobs),
            None => Ok(()),
        }
    }
}

/// 换成真实的位置；换不成的照原样，助手会再换一次，换不成就不跑。
fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests;
