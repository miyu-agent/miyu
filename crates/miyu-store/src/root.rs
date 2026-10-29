//! 数据根和缓存目录（`docs/designs/07-存储.md` 第二节）：在哪，第一次用时怎么建。
//!
//! 数据根装着全部真相和派生数据，一个数据根上只跑一个核心，默认在家目录的 `.miyu` 里；缓存目录
//! 装模型文件这类大缓存，整台机器共用，换了数据根也不用重新下载。两样都照一份环境快照（[`Env`]）
//! 找。数据根的顶层有一个标记文件，认不出是自己的数据根就不碰它：家目录里的 `.miyu` 不一定是
//! 我们建的。

use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use miyu_kernel::id::SessionId;

use crate::durable::{create_dir, create_dir_anywhere, sync_dir};
use crate::env::{Env, Platform};

/// 自己放东西的几处（[`DataRoot::own_places`]）。
const OWN_PLACES: [&str; 4] = ["scripts", "skills", "commands", "packages"];

/// 照 XDG 用户目录给主人的几处（施工 Y-11 上）：她替人做出来、拿到的东西放这里，用到的时候才建。名字照 XDG 的英文，界面上照
/// 语言显示。
pub const USER_DIRS: [&str; 4] = ["pictures", "videos", "documents", "downloads"];

/// 数据根 `data_root` 里照 XDG 的那一处 `name`（[`USER_DIRS`] 里的一个）：`<name>/`（施工 Y-11 下；施工 SU-5 上起直接在数据根
/// 下）。调的一方手里只有数据根的路径时用它（调扩展的工具、脚本工具），和 [`DataRoot::user_dirs`] 同一个位置。不建。
pub fn user_dir(data_root: &Path, name: &str) -> PathBuf {
    data_root.join(name)
}

/// 缓存（施工 Y-11 上）：`cache/`，能丢的放这里，30 天没碰的清掉（`crate::cache`）。
pub const CACHE: &str = "cache";

/// 第一次用时建的两个顶层目录：状态区、运行时（`07-存储.md` 第二节；施工 SU-5 上起没有系统区、家目录两层）。别的用到时再建。
const SKELETON: [&str; 2] = ["state", "run"];

/// 标记文件：它在，这个目录才是 Miyu 的数据根（`07-存储.md` 第二节「认得出自己的数据根才动它」）。装沙盒时也照它
/// 认给的数据根（`docs/blueprint/sandbox/windows.md`，施工 5-8）。
pub const MARKER: &str = ".miyu-root";

/// 标记文件里写的一行：给翻到它的人看。现在只认文件在不在。
const MARKER_TEXT: &str = "This directory is a Miyu data root (layout 2).\n";

/// 找不到数据根、缓存目录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootError {
    /// `MIYU_HOME` 写的是相对路径：它跟着当前目录变，同一个人在两个目录里启动，会找到两个
    /// 数据根。
    RelativeMiyuHome(PathBuf),
    /// 家目录找不到（Linux、macOS）。
    NoHome,
    /// `LOCALAPPDATA` 没有，或者不是绝对路径（Windows 的缓存目录要它）。
    NoLocalAppData,
}

impl fmt::Display for RootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RootError::RelativeMiyuHome(path) => {
                write!(f, "MIYU_HOME 要写绝对路径，写的是 {}", path.display())
            }
            RootError::NoHome => write!(f, "找不到家目录"),
            RootError::NoLocalAppData => write!(f, "找不到 LOCALAPPDATA，或者它不是绝对路径"),
        }
    }
}

impl std::error::Error for RootError {}

/// 一个数据根。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRoot {
    path: PathBuf,
}

impl DataRoot {
    /// 照快照找数据根：`MIYU_HOME` 设了就是它（开头的 `~` 照家目录接，[`Env::expand`]），不然是家目录的
    /// `.miyu`，三个平台一样（`07-存储.md` 第二节「默认位置」「怎么找」）。
    ///
    /// # Errors
    ///
    /// `MIYU_HOME` 是相对路径；要用家目录时找不到。
    pub fn locate(env: &Env) -> Result<DataRoot, RootError> {
        if let Some(miyu_home) = set(&env.miyu_home) {
            let path = env.expand(miyu_home).ok_or(RootError::NoHome)?;
            return match path.is_absolute() {
                true => Ok(DataRoot { path }),
                false => Err(RootError::RelativeMiyuHome(path)),
            };
        }
        Ok(DataRoot {
            path: home(env)?.join(".miyu"),
        })
    }

    /// 数据根本身。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 装上的软件包：`packages/`（施工 SU-5 上：原来 `packages/`）。
    pub fn packages(&self) -> PathBuf {
        self.path.join("packages")
    }

    /// 自己放脚本、技能、命令模板、装包的几处（`scripts`、`skills`、`commands`、`packages`）的真实位置（施工 X-37、X-37 补）：
    /// 找的时候认链接指到这几处下面的，管理员放行出沙盒的文件得落在这几处下面。照数据根的真实位置接上，不一处处换成真实位置：
    /// `skills/` 本身是指到别处的链接时，换了就把别处当成自己的地方了。数据根本身是链接的（macOS 的临时目录在 `/var` 下面）照样
    /// 对。碰磁盘。
    pub fn own_places(&self) -> Vec<PathBuf> {
        let base = self.real();
        OWN_PLACES.iter().map(|dir| base.join(dir)).collect()
    }

    /// 装上的包（[`DataRoot::packages`]）的真实位置（施工 U23 上上）：主人装的，放行出沙盒的文件落在这下面的也算管得住（同出厂
    /// 的包）。碰磁盘。
    pub fn real_packages(&self) -> PathBuf {
        self.real().join("packages")
    }

    /// 数据根的真实位置（还没有的照原样）。碰磁盘。
    fn real(&self) -> PathBuf {
        std::fs::canonicalize(&self.path).unwrap_or_else(|_| self.path.clone())
    }

    /// 建工作区，已经有的不动：核心起来时建（施工 3-9 上；工作区施工 4-3 下）。Unix 上新建的权限 0700。
    ///
    /// # Errors
    ///
    /// 建不了；该是目录的地方是个文件。
    pub fn prepare_workspace(&self) -> io::Result<()> {
        create_dir(&self.workspace())
    }

    /// [`USER_DIRS`] 那几处：名字和位置，照 [`USER_DIRS`] 的先后。不建。
    pub fn user_dirs(&self) -> Vec<(&'static str, PathBuf)> {
        USER_DIRS
            .iter()
            .map(|name| (*name, user_dir(&self.path, name)))
            .collect()
    }

    /// 缓存：`cache/`。不建。
    pub fn cache(&self) -> PathBuf {
        self.path.join(CACHE)
    }

    /// 工作区：`workspace/`（`07-存储.md` 第二节）。头报来的工作目录太宽时退回这里（`11-权限与沙盒.md` 第四节，施工 4-3 下）。
    pub fn workspace(&self) -> PathBuf {
        self.path.join("workspace")
    }

    /// 一个会话的默认工作区：`workspace/<会话编号>/`（施工 5-14，`11-权限与沙盒.md` 第四节「默认工作区每个会话一个」）。网页不选
    /// 目录的、工作目录太宽的照它；子会话、分叉出来的用父会话、来源会话的那一个（`session.created` 的 `workspace_from`）。不在这里
    /// 建：用到的一方自己建。
    pub fn session_workspace(&self, session: &SessionId) -> PathBuf {
        self.workspace().join(session.as_str())
    }

    /// 有哪些会话，从新到旧：会话编号是 UUIDv7，照编号倒着排就是照造的先后倒着（施工 3-9 下）。不合会话编号写法的目录不算；还
    /// 没有会话的是空的。
    ///
    /// # Errors
    ///
    /// 读不了目录。
    pub fn sessions(&self) -> io::Result<Vec<SessionId>> {
        let entries = match fs::read_dir(self.path.join("sessions")) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let mut sessions = Vec::new();
        for entry in entries {
            let name = entry?.file_name();
            if let Some(id) = name.to_str().and_then(|name| SessionId::parse(name).ok()) {
                sessions.push(id);
            }
        }
        sessions.sort_by(|a, b| b.as_str().cmp(a.as_str()));
        Ok(sessions)
    }

    /// 一个会话的目录：`sessions/<会话编号>/`（`07-存储.md` 第三节）。
    pub fn session_dir(&self, session: &SessionId) -> PathBuf {
        self.path.join("sessions").join(session.as_str())
    }

    /// 删掉的会话放在哪：回收处 `trash/sessions/`，一个会话一个目录，名字是会话编号（施工 3-8 三补，[`crate::trash`]）。
    pub fn trashed_sessions(&self) -> PathBuf {
        self.path.join("trash").join("sessions")
    }

    /// 删掉的人格：`trash/personas/`（施工 P-3 下，`crate::trash::personas`）。
    pub fn trashed_personas(&self) -> PathBuf {
        self.path.join("trash").join("personas")
    }

    /// blob：`blobs/`（`07-存储.md` 第五节）。
    pub fn blobs(&self) -> PathBuf {
        self.path.join("blobs")
    }

    /// 派生数据：`index/`（`07-存储.md` 第六节）。会话列表的索引放在这里（施工 3-8 七补，[`crate::index`]）。
    pub fn index(&self) -> PathBuf {
        self.path.join("index")
    }

    /// 状态区：派生的全局索引、用量总表、运行日志。
    pub fn state(&self) -> PathBuf {
        self.path.join("state")
    }

    /// 运行时：锁文件、套接字、本机令牌。
    pub fn run(&self) -> PathBuf {
        self.path.join("run")
    }

    /// 建骨架：先认标记。目录不存在、是空的，先写下标记；有标记的照常；不是空的又没有标记的，
    /// 认不出是 Miyu 的数据根，里面什么都不建。然后两个顶层目录，缺的才建，建两次也不出错。
    ///
    /// 两个进程同时第一次用这个数据根（两个头同时拉起核心），也都成（施工 3-9 上）：别处刚写下的
    /// 标记也算。
    ///
    /// Unix 上新建的权限 0700，只有本人能进；已经有的不改：数据根可能是人自己建、自己设的，权限
    /// 不对由 `miyu doctor` 报告（`22-命令行.md` 第五节）。Windows 上靠用户目录本身的访问控制。
    ///
    /// # Errors
    ///
    /// 认不出是 Miyu 的数据根；建不了目录、写不了标记，或者该是目录的地方是个文件。
    pub fn prepare(&self) -> Result<(), PrepareError> {
        // 数据根本身还没有标记：照不认标记的建（施工 TMP-1）。
        create_dir_anywhere(&self.path)?;
        let marker = self.path.join(MARKER);
        if fs::symlink_metadata(&marker).is_err() {
            if fs::read_dir(&self.path)?.next().is_some() {
                // 不是空的：可能是别处刚写下了标记，再看一眼。标记总是先于骨架写下，有了骨架就有标记。
                if fs::symlink_metadata(&marker).is_err() {
                    return Err(PrepareError::NotOurs(self.path.clone()));
                }
            } else {
                match fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&marker)
                {
                    Ok(mut file) => {
                        file.write_all(MARKER_TEXT.as_bytes())?;
                        file.sync_all()?;
                        // 同步数据根，标记这一项才算落盘：断电以后标记没了、骨架还在，下次就认不出自己了。
                        sync_dir(&self.path)?;
                    }
                    // 别处刚写下了标记。
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
        for name in SKELETON {
            create_dir(&self.path.join(name))?;
        }
        Ok(())
    }
}

/// 在数据根里建目录，连同缺的上层（施工 TMP-1）：已经有的不动；要建的，往上找得到数据根的标记（[`MARKER`]）才建。别的 crate
/// 往数据根里写文件（缓存、生成的说明）先照它建目录：数据根被删了，后台写的那一下不把它建回来。
///
/// # Errors
///
/// 往上找不到数据根的标记（`NotFound`，说的是哪个目录）；建不了；该是目录的地方是个文件；同步不了。
pub fn create_inside(dir: &Path) -> io::Result<()> {
    crate::durable::create_dir(dir)
}

/// 建骨架建不成。
#[derive(Debug)]
pub enum PrepareError {
    /// 目录里有别的东西，又没有标记：认不出是 Miyu 的数据根，一个字节都不动它。里面是什么不去猜，
    /// 只有这一种说法。
    NotOurs(PathBuf),
    /// 读写出错。
    Io(io::Error),
}

impl fmt::Display for PrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrepareError::NotOurs(path) => write!(
                f,
                "{} 里有别的东西，认不出是 Miyu 的数据根（顶层没有 {MARKER}），不动它。设 MIYU_HOME 指到一个空目录",
                path.display()
            ),
            PrepareError::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for PrepareError {}

impl From<io::Error> for PrepareError {
    fn from(error: io::Error) -> PrepareError {
        PrepareError::Io(error)
    }
}

/// 缓存目录：整台机器共用，不跟着 `MIYU_HOME` 变（`07-存储.md` 第二节）。只找，不建：用到它的
/// 到时候建。
///
/// # Errors
///
/// 要用家目录、`LOCALAPPDATA` 时找不到。
pub fn cache_root(env: &Env) -> Result<PathBuf, RootError> {
    Ok(match env.platform {
        Platform::Linux => match absolute(&env.xdg_cache_home) {
            Some(cache) => cache.join("miyu"),
            None => home(env)?.join(".cache").join("miyu"),
        },
        Platform::Macos => home(env)?.join("Library").join("Caches").join("Miyu"),
        Platform::Windows => local_app_data(env)?.join("Miyu").join("cache"),
    })
}

/// 设了、不是空的。
fn set(value: &Option<OsString>) -> Option<&OsString> {
    value.as_ref().filter(|value| !value.is_empty())
}

/// 设了、是绝对路径的：XDG 规范说相对路径的不算。
fn absolute(value: &Option<OsString>) -> Option<PathBuf> {
    set(value)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

/// 家目录，要是绝对路径。
fn home(env: &Env) -> Result<&Path, RootError> {
    env.home
        .as_deref()
        .filter(|home| home.is_absolute())
        .ok_or(RootError::NoHome)
}

/// `LOCALAPPDATA`，要是绝对路径。
fn local_app_data(env: &Env) -> Result<PathBuf, RootError> {
    absolute(&env.local_app_data).ok_or(RootError::NoLocalAppData)
}

#[cfg(test)]
mod tests;
