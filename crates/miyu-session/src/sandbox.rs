//! 一次调用带的沙盒（`docs/blueprint/session/tools.md` 第 1a 条，施工 5-4 上）：照派出去那一刻实际生效的那一级写
//! 规格。完全放开不进沙盒；工作区能写这一轮的工作目录、临时目录，工具链的缓存用沙盒自己的一份（[`caches`]，施工 5-4
//! 下）；只读哪儿都不能写；两级都藏数据根和别的程序的私密目录（施工 5-13）。

mod caches;
mod confined;

pub use caches::SandboxCache;

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use miyu_fs::{private_dirs, resolve, within};
use miyu_kernel::event::Permission;
use miyu_kernel::raw::RawJson;
use miyu_sandbox::{Sandboxed, Spec};

use crate::default_workspace::DefaultWorkspace;
use crate::guard::{Effective, effective};

/// 给调用写沙盒要的：助手在哪、系统的家目录、数据根、沙盒的缓存。一个会话一份，这台机器上的沙盒能用才有。
#[derive(Debug, Clone)]
pub(crate) struct Sandbox {
    helper: PathBuf,
    home: Option<PathBuf>,
    data_root: PathBuf,
    cache: Option<SandboxCache>,
    /// 外部身份的会话（施工 5-12 下）：读写都只限工作区（`confined.rs`）。
    confined: bool,
    /// 开着的技能文件夹（施工 X-10 中）：数据根藏着，它们单独放开只读。
    shelves: Vec<PathBuf>,
    /// 这个会话的附件目录（施工 3-9 六补）：同技能文件夹单独放开只读；还没建（没存过附件）的不放：Landlock 认不得
    /// 不在的路径。
    attachments: Option<PathBuf>,
    /// 工作目录太宽时退回哪（施工 5-14）：没有的能写的就是工作目录。
    workplace: Option<DefaultWorkspace>,
    /// 这一轮的私密目录（施工 5-15 下）：回合开始照配置换；还没换过的照出厂的。
    private: Option<Vec<PathBuf>>,
}

impl Sandbox {
    /// 助手是 `helper`，`~` 照 `home` 换，藏的是 `data_root`，工具链的缓存放在 `cache`（没有的不设）；外部身份的会话
    /// `confined`（施工 5-12 下）。
    pub(crate) fn new(
        helper: PathBuf,
        home: Option<PathBuf>,
        data_root: PathBuf,
        (cache, confined): (Option<SandboxCache>, bool),
    ) -> Sandbox {
        Sandbox {
            helper,
            home,
            data_root,
            cache,
            confined,
            shelves: Vec::new(),
            attachments: None,
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
    /// 完全放开的不带。本会话放行过的规则 `grants` 里放行写的目录也能写（施工 5-15，「此目录不再询问」）。碰磁盘（换真实的
    /// 位置、建沙盒自己的临时目录），在阻塞线程里调。
    ///
    /// # Errors
    ///
    /// 数据根落在临时目录里，沙盒自己的临时目录建不成；沙盒的缓存建不成、链接建不成；有了却不是只给本人的。
    pub(crate) fn for_call(
        &self,
        permission: &Permission,
        cwd: &str,
        dirs: &[String],
        grants: &[RawJson],
    ) -> io::Result<Option<Sandboxed>> {
        // 外部身份的会话不看级别，读写都只限工作区（施工 5-12 下）。
        if self.confined {
            return self.confined_call(permission, cwd, dirs).map(Some);
        }
        let level = effective(permission);
        if level == Effective::Full {
            return Ok(None);
        }
        let data_root = real(&self.data_root);
        let mut env = Vec::new();
        let write = if level == Effective::Workspace {
            // 工作目录照权限策略的办法换（头报来的可能是 `~`），换不成的照原样。
            let home = self.home.as_deref();
            let cwd = resolve(Path::new(cwd), home, cwd).unwrap_or_else(|_| PathBuf::from(cwd));
            let temp = real(&std::env::temp_dir());
            // 数据根在临时目录里：藏的落在能写的里面，挖不了洞。不放整个临时目录，放沙盒自己的一个。
            let temp = if data_root.starts_with(&temp) {
                let own = own_temp(&data_root)?;
                env.push((OsString::from("TMPDIR"), own.clone().into_os_string()));
                own
            } else {
                temp
            };
            // 能写的是工作区：太宽的工作目录退回这个会话的默认工作区，用到时建（施工 5-14）。
            let workspace = self.workspace(&cwd, &data_root)?;
            // 加进来的目录排在工作区后面，照同一个办法换（施工 5-10 上）。
            let mut write = vec![workspace];
            write.extend(dirs.iter().map(|dir| {
                resolve(Path::new(dir), home, dir).unwrap_or_else(|_| PathBuf::from(dir))
            }));
            write.extend(granted_writes(grants));
            write.push(temp);
            if let Some(cache) = &self.cache {
                let (dir, variables) = cache.prepare(&data_root)?;
                write.push(dir);
                env.extend(variables);
            }
            write
        } else {
            Vec::new()
        };
        // 有能用的挂载（施工 5-16）：能写的里面的私密目录也藏，助手用挂载盖住；没有的照旧不藏（Landlock 挖不掉，施工 5-13）。
        let mount = miyu_sandbox::mounts();
        let hidden = self.hidden(data_root, &write, mount);
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
                mount,
                // 脚本调工具的通道由 `shell` 起命令时加（施工 M-3）。
                sockets: Vec::new(),
            },
            env,
        }))
    }
}

impl Sandbox {
    /// 藏起来的：数据根，加这台机器上有的别的程序的私密目录（施工 5-13，11 第四节；这一轮的配置加的、去掉的照它，施工 5-15 下）。私密目录落在能写的 `write` 里面的
    /// 只在能用挂载时（`mount`，施工 5-16）藏，不然照人给的算（同权限策略的先后），Landlock 一级级绕开会冻住工作区那一层。
    fn hidden(&self, data_root: PathBuf, write: &[PathBuf], mount: bool) -> Vec<PathBuf> {
        let mut hidden = vec![data_root];
        let private = match &self.private {
            Some(private) => private.clone(),
            None => self.home.as_deref().map(private_dirs).unwrap_or_default(),
        };
        for dir in private
            .iter()
            .filter_map(|dir| std::fs::canonicalize(dir).ok())
        {
            if !hidden.contains(&dir) && (mount || !write.iter().any(|base| within(&dir, base))) {
                hidden.push(dir);
            }
        }
        hidden
    }
}

impl Sandbox {
    /// 真实位置是 `cwd` 的工作目录对应的工作区（施工 5-14）：退回默认工作区的，还没建的建上（Landlock 认不得不在的路径）。
    fn workspace(&self, cwd: &Path, data_root: &Path) -> io::Result<PathBuf> {
        let Some(workplace) = &self.workplace else {
            return Ok(cwd.to_path_buf());
        };
        let home = self.home.as_deref().map(real);
        workplace.made(cwd, home.as_deref(), data_root)
    }
}

/// 放行过的规则里放行写的目录（施工 5-15）：每条规则的 `write`（问人时写的就是真实的位置）。读不懂的规则不算，同权限策略。
fn granted_writes(grants: &[RawJson]) -> Vec<PathBuf> {
    grants
        .iter()
        .filter_map(|rule| serde_json::from_str::<serde_json::Value>(rule.get()).ok())
        .filter_map(|rule| {
            rule.get("write")
                .and_then(serde_json::Value::as_array)
                .cloned()
        })
        .flatten()
        .filter_map(|scope| scope.as_str().map(PathBuf::from))
        .collect()
}

/// 换成真实的位置；换不成的照原样，助手会再换一次，换不成就不跑。
fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// 沙盒自己的临时目录：数据根旁边的 `<数据根>-sandbox-tmp`，没有就建，只给本人。
fn own_temp(data_root: &Path) -> io::Result<PathBuf> {
    let mut name = data_root
        .file_name()
        .map(OsString::from)
        .unwrap_or_default();
    name.push("-sandbox-tmp");
    let dir = data_root.with_file_name(name);
    private_dir(&dir, data_root)?;
    Ok(dir)
}

/// 建只给本人的目录（0700，没有的上级一起建）；建好的、已经有的，要是目录、不是链接、和数据根 `owner_of` 同一个属主，组和
/// 别人一点权限都没有。
#[cfg(unix)]
fn private_dir(dir: &Path, owner_of: &Path) -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};

    match std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
    {
        Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Err(error),
        _ => {}
    }
    let found = std::fs::symlink_metadata(dir)?;
    let owner = std::fs::metadata(owner_of)?.uid();
    if found.is_dir() && found.uid() == owner && found.mode() & 0o077 == 0 {
        Ok(())
    } else {
        Err(not_private(dir))
    }
}

/// 建目录（没有的上级一起建）；建好的、已经有的，要是目录、不是链接。临时目录、缓存目录都在本人的用户目录里，别人
/// 进不来。
#[cfg(windows)]
fn private_dir(dir: &Path, _owner_of: &Path) -> io::Result<()> {
    match std::fs::create_dir_all(dir) {
        Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Err(error),
        _ => {}
    }
    if std::fs::symlink_metadata(dir)?.is_dir() {
        Ok(())
    } else {
        Err(not_private(dir))
    }
}

/// 有了却不是只给本人的。
fn not_private(dir: &Path) -> io::Error {
    io::Error::other(format!("{} is not a private directory", dir.display()))
}

#[cfg(test)]
mod tests;
