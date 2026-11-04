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

/// 你自己放脚本、技能、命令模板的几处，在你的家里（[`DataRoot::own_places`]）。
const OWN_PLACES: [&str; 3] = ["scripts", "skills", "commands"];

/// 数据根这一层的几块（设计 `55-数据根仿FHS.md` 第二节）。
const ETC: &str = "etc";
/// 凭据，在 `etc/` 下面。
const PRIVATE_DIR: &str = "private";
/// 卸掉的出厂包的记号，在 `etc/` 下面。
pub const REMOVED: &str = "removed";
/// 装上的软件包。
const OPT: &str = "opt";
/// 你的家：只给一个人用，这个人就是 root（以后多用户另开 `home/<用户名>/`）。
const USER: &str = "root";
/// 机器的状态、缓存、日志、跨重启的暂存。
const VAR: &str = "var";
/// 短命的暂存。
const TMP: &str = "tmp";

/// 核心自己在 `etc/`、`var/lib/`、`var/cache/`、`var/log/`、`tmp/`、`run/` 这几层用到的名字（设计 55 第三节第 3 条）：包各占
/// 一个 `<编号>/`，这些不能用作包的编号。表在 `miyu_config::package`（施工 FH-5下：软件源收录也查）；测试照各层的方法推一遍，
/// 和这张表对。
pub use miyu_config::package::RESERVED;

/// 照 XDG 用户目录给主人的几处（施工 Y-11 上）：她替人做出来、拿到的东西放这里，用到的时候才建。名字照 XDG 的英文，界面上照
/// 语言显示。
pub const USER_DIRS: [&str; 4] = ["pictures", "videos", "documents", "downloads"];

/// 运行时：`run/`，锁文件、套接字、本机令牌；写数据根里的配置、密钥时的临时文件也建在这里（施工 R3-1，
/// `crate::config_file`）。
pub const RUN: &str = "run";

/// 核心起来时就建好的目录（施工 R3-1 补、FH-3，[`DataRoot::prepare_private`]，设计 55 第五节第 2 条）：Windows 的沙盒只禁
/// 命令起来那一刻已有的，敏感的、透出路径上的几层都事先建好。
const PRIVATE: [&str; 21] = [
    "etc",
    "etc/private",
    "etc/removed",
    "opt",
    "root",
    "root/sessions",
    "root/memory",
    "root/kb",
    "root/blobs",
    "root/trash",
    "root/state",
    "root/workspace",
    "var/lib",
    "var/lib/packages",
    "var/lib/mcp",
    "var/cache",
    "var/cache/index",
    "var/log",
    "var/tmp",
    "tmp",
    RUN,
];

/// 第一次用时建的：配置和凭据（建数据根时就在，Windows 沙盒里整条禁，凭据第一次新建时也在禁止的目录里）、运行时、短命的暂存
/// （设计 55 第五节第 1 条）。别的核心起来时建（[`PRIVATE`]）。
const SKELETON: [&str; 4] = ["etc", "etc/private", RUN, "tmp"];

/// 标记文件：它在，这个目录才是 Miyu 的数据根（`07-存储.md` 第二节「认得出自己的数据根才动它」）。装沙盒时也照它认给的数据根
/// （`docs/blueprint/sandbox/windows.md`，施工 5-8）。布局 3 起换了名字（施工 FH-3，设计 55 第六节第 1 条）：旧的程序只认
/// [`OLD_MARKER`]，碰到新的数据根认不出、不往里写。
pub const MARKER: &str = ".miyu-layout";

/// 以前（布局 1、2）的标记文件：有它、没有 [`MARKER`] 的是旧版的数据根，新版不认、不动。
pub const OLD_MARKER: &str = ".miyu-root";

/// 标记文件里写的一行，带布局的版本：读的时候认它。
pub const MARKER_TEXT: &str = "This directory is a Miyu data root (layout 3).\n";

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

    /// 已经找好的数据根（施工 FH-2）：工具、脚本拿到的是数据根的路径（`Call::data_root`），照它找里面的位置。不认标记、不建。
    pub fn at(path: PathBuf) -> DataRoot {
        DataRoot { path }
    }

    /// 数据根本身。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 装上的软件包：`opt/`（设计 55 第二节）。
    pub fn packages(&self) -> PathBuf {
        self.path.join(OPT)
    }

    /// 自己放脚本、技能、命令模板的几处（`root/scripts`、`root/skills`、`root/commands`）和装包的 `opt/` 的真实位置（施工 X-37、X-37 补）：
    /// 找的时候认链接指到这几处下面的，主人放行出沙盒的文件得落在这几处下面。照数据根的真实位置接上，不一处处换成真实位置：
    /// `skills/` 本身是指到别处的链接时，换了就把别处当成自己的地方了。数据根本身是链接的（macOS 的临时目录在 `/var` 下面）照样
    /// 对。碰磁盘。
    pub fn own_places(&self) -> Vec<PathBuf> {
        let base = self.real();
        OWN_PLACES
            .iter()
            .map(|dir| base.join(USER).join(dir))
            .chain(std::iter::once(base.join(OPT)))
            .collect()
    }

    /// 装上的包（[`DataRoot::packages`]）的真实位置（施工 U23 上上）：主人装的，放行出沙盒的文件落在这下面的也算管得住（同出厂
    /// 的包）。碰磁盘。
    pub fn real_packages(&self) -> PathBuf {
        self.real().join(OPT)
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

    /// 建 `PRIVATE`（设计 55 第五节第 2 条那张表）那几处，已经有的不动：核心起来时建（施工 R3-1 补、FH-3）。Windows 的
    /// 沙盒只禁命令起来那一刻已有的（施工 SW-6），命令跑着时第一次建的整棵读得到。Unix 上新建的权限 0700。
    ///
    /// # Errors
    ///
    /// 建不了；该是目录的地方是个文件。
    pub fn prepare_private(&self) -> io::Result<()> {
        for name in PRIVATE {
            create_dir(&self.path.join(name))?;
        }
        Ok(())
    }

    /// 配置和凭据：`etc/`。
    fn etc(&self) -> PathBuf {
        self.path.join(ETC)
    }

    /// 凭据：`etc/private/`。
    fn private(&self) -> PathBuf {
        self.etc().join(PRIVATE_DIR)
    }

    /// 你的家：`root/`。
    fn user(&self) -> PathBuf {
        self.path.join(USER)
    }

    /// 机器和包的状态：`var/lib/`。
    fn var_lib(&self) -> PathBuf {
        self.path.join(VAR).join("lib")
    }

    /// 能删的缓存：`var/cache/`。
    fn var_cache(&self) -> PathBuf {
        self.path.join(VAR).join("cache")
    }

    /// 运行日志：`var/log/`。
    fn var_log(&self) -> PathBuf {
        self.path.join(VAR).join("log")
    }

    /// 短命的暂存：`tmp/`（设计 55 第三节第 5 条）。
    fn tmp(&self) -> PathBuf {
        self.path.join(TMP)
    }

    /// [`USER_DIRS`] 那几处：名字和位置，照 [`USER_DIRS`] 的先后。不建。
    pub fn user_dirs(&self) -> Vec<(&'static str, PathBuf)> {
        USER_DIRS
            .iter()
            .map(|name| (*name, self.user_dir(name)))
            .collect()
    }

    /// 工作区：`root/workspace/`（`07-存储.md` 第二节，设计 55）。头报来的工作目录太宽时退回这里（`11-权限与沙盒.md` 第四节，施工 4-3 下）。
    pub fn workspace(&self) -> PathBuf {
        self.user().join("workspace")
    }

    /// 一个会话的默认工作区：`root/workspace/<会话编号>/`（施工 5-14，`11-权限与沙盒.md` 第四节「默认工作区每个会话一个」）。网页不选
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
        let entries = match fs::read_dir(self.user().join("sessions")) {
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

    /// 一个会话的目录：`root/sessions/<会话编号>/`（`07-存储.md` 第三节）。
    pub fn session_dir(&self, session: &SessionId) -> PathBuf {
        self.user().join("sessions").join(session.as_str())
    }

    /// 会话都在哪：`root/sessions/`（施工 SE-1 下：导入先在这里暂存、核对完再改名进位置）。
    pub fn sessions_dir(&self) -> PathBuf {
        self.user().join("sessions")
    }

    /// 导进来的会话的标记（施工 SE-1 下，设计 58 第二节第 6 条）：`root/sessions/<会话编号>/imported.json`，记从哪个包来、
    /// 是不是只读存档；不是导进来的没有这个文件。删会话时跟着会话目录一起进回收处。
    pub fn imported_file(&self, session: &SessionId) -> PathBuf {
        self.session_dir(session).join(crate::bundle::MARKER)
    }

    /// 删掉的会话放在哪：回收处 `root/trash/sessions/`，一个会话一个目录，名字是会话编号（施工 3-8 三补，[`crate::trash`]）。
    pub fn trashed_sessions(&self) -> PathBuf {
        self.user().join("trash").join("sessions")
    }

    /// 删掉的人格：`root/trash/personas/`（施工 P-3 下，`crate::trash::personas`）。
    pub fn trashed_personas(&self) -> PathBuf {
        self.user().join("trash").join("personas")
    }

    /// blob：`root/blobs/`（`07-存储.md` 第五节）。
    pub fn blobs(&self) -> PathBuf {
        self.user().join("blobs")
    }

    /// 派生数据：`var/cache/index/`（`07-存储.md` 第六节）。会话列表的索引放在这里（施工 3-8 七补，[`crate::index`]）。
    pub fn index(&self) -> PathBuf {
        self.var_cache().join("index")
    }

    /// 运行时：锁文件、套接字、本机令牌。
    pub fn run(&self) -> PathBuf {
        self.path.join(RUN)
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
                    // 旧版的数据根（施工 FH-3，设计 55 第六节第 1 条）：不认、不动，说清楚是旧版的。
                    if fs::symlink_metadata(self.path.join(OLD_MARKER)).is_ok() {
                        return Err(PrepareError::OldLayout(self.path.clone()));
                    }
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
        // 标记写的布局要是这一版的：以后的布局（数字更大）、写坏了的都不认（施工 FH-3）。
        if marker_text(&marker) != MARKER_TEXT {
            return Err(PrepareError::OtherLayout(self.path.clone()));
        }
        for name in SKELETON {
            create_dir(&self.path.join(name))?;
        }
        Ok(())
    }
}

/// 标记里写的字。空的多看几眼（最多半秒）：别处刚只许新建地建了标记、还没写进字（两个头同时第一次拉起核心）。
fn marker_text(marker: &Path) -> String {
    for _ in 0..50 {
        let text = fs::read_to_string(marker).unwrap_or_default();
        if !text.is_empty() {
            return text;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    String::new()
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
    /// 目录里有别的东西，又没有标记：认不出是 Miyu 的数据根，一个字节都不动它。里面是什么不去猜。
    NotOurs(PathBuf),
    /// 有旧版的标记（[`OLD_MARKER`]）、没有这一版的：旧版 Miyu 的数据根（布局 2），新版不认、不动（施工 FH-3，设计 55 第六节
    /// 第 1 条）。
    OldLayout(PathBuf),
    /// 标记写的不是这一版的布局（以后的版本、写坏了的）：不认、不动。
    OtherLayout(PathBuf),
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
            PrepareError::OldLayout(path) => write!(
                f,
                "{} 是旧版 Miyu 的数据根（layout 2），新版不认；设 MIYU_HOME 指到一个空目录，旧数据照施工单 FH-7 自己搬",
                path.display()
            ),
            PrepareError::OtherLayout(path) => write!(
                f,
                "{} 的 {MARKER} 写的不是这一版的布局（layout 3），不动它",
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

mod places;
mod tmp;

pub use tmp::{STALE_VAR_TMP, Swept};

#[cfg(test)]
mod tests;
