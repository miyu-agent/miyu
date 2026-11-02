//! 交给 Miyu 拉起来的程序的环境变量（施工 FH-2下，设计 `55-数据根仿FHS.md` 第四节）：扩展、包带的命令、脚本拿到的几处目录叫
//! 什么名字、指到哪，只在这里定；拉起程序的几处（扩展、`miyu <包的命令>`、脚本工具）都照这里给。
//!
//! 每一种目录交的是一串名字（[`Names`]）：同一个位置同时用几个名字交出去。FH-3下 起照设计 55 第四节给新名字（`MIYU_STATE_DIR`
//! 这些），旧名字过渡期间一起给、指到同一个位置，FH-6 删旧名字时只改这里。

use std::io;
use std::path::{Path, PathBuf};

use crate::root::{DataRoot, USER_DIRS};

/// 同一个位置交出去的几个名字。
pub type Names = &'static [&'static str];

/// 数据根。
pub const HOME: Names = &["MIYU_HOME"];

/// 资源目录。
pub const RESOURCES: Names = &["MIYU_RESOURCES"];

/// 包的文件夹（施工 S-6 上）：出厂的在资源目录，装上的在 `opt/`，由核心挑；包带的脚本是它所在的包的文件夹。
/// `MIYU_PACKAGE_DIR` 是旧名字（FH-6 删）。
pub const APP: Names = &["MIYU_APP_DIR", "MIYU_PACKAGE_DIR"];

/// 包自己的配置：`etc/<编号>/`。
pub const CONFIG: Names = &["MIYU_CONFIG_DIR"];

/// 包自己的状态（施工 F-15）：`var/lib/<编号>/`，扩展的工作目录也是它。`MIYU_PACKAGE_STATE` 是旧名字（FH-6 删）。
pub const PACKAGE_STATE: Names = &["MIYU_STATE_DIR", "MIYU_PACKAGE_STATE"];

/// 包自己能删的缓存：`var/cache/<编号>/`。
pub const CACHE: Names = &["MIYU_CACHE_DIR"];

/// 包自己的日志：`var/log/<编号>/`（核心记的标准错误也在这里，叫 `stderr`）。
pub const LOG: Names = &["MIYU_LOG_DIR"];

/// 包自己的短命暂存：`tmp/<编号>/`，拉起前清。
pub const TMP: Names = &["MIYU_TMP_DIR"];

/// 包自己的运行时目录：`run/<编号>/`（锁、套接字、地址）。
pub const RUNTIME: Names = &["MIYU_RUNTIME_DIR"];

/// 脚本的状态目录（施工 X-44）。`MIYU_SCRIPT_STATE_DIR` 是旧名字（FH-6 删）。
pub const SCRIPT_STATE: Names = &["MIYU_STATE_DIR", "MIYU_SCRIPT_STATE_DIR"];

/// 脚本的缓存目录（施工 Y-11 上）。`MIYU_SCRIPT_CACHE_DIR` 是旧名字（FH-6 删）。
pub const SCRIPT_CACHE: Names = &["MIYU_CACHE_DIR", "MIYU_SCRIPT_CACHE_DIR"];

/// 脚本交图的目录（施工 X-35）。`MIYU_SCRIPT_IMAGES_DIR` 是旧名字（FH-6 删）。
pub const SCRIPT_IMAGES: Names = &["MIYU_IMAGES_DIR", "MIYU_SCRIPT_IMAGES_DIR"];

/// 命令型 MCP 服务的状态目录：`var/lib/mcp/<服务>/`，也是它的工作目录。
pub const MCP_STATE: Names = &["MIYU_STATE_DIR"];

/// 照 XDG 给你的那一处 `name`（[`crate::root::USER_DIRS`] 里的）交出去的名字：照名字配，不靠先后。不认识的没有：
/// `USER_DIRS` 加了一处，这里不补就不交，测试当场报红。
pub fn user_dir(name: &str) -> Option<Names> {
    match name {
        "pictures" => Some(&["MIYU_PICTURES_DIR"]),
        "videos" => Some(&["MIYU_VIDEOS_DIR"]),
        "documents" => Some(&["MIYU_DOCUMENTS_DIR"]),
        "downloads" => Some(&["MIYU_DOWNLOADS_DIR"]),
        _ => None,
    }
}

/// 名字表 `names` 里每一个都指到 `value`。
pub fn named(names: Names, value: &Path) -> impl Iterator<Item = (&'static str, PathBuf)> + '_ {
    names.iter().map(move |name| (*name, value.to_path_buf()))
}

/// 一个包拉起时交给它的几处（设计 55 第四节那一张表的「包」一列）：包的文件夹，它在 `etc/`、`var/lib/`、`var/cache/`、
/// `var/log/`、`tmp/`、`run/` 各自的那一格，照 XDG 给你的几处。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDirs {
    /// 包的文件夹。
    pub app: PathBuf,
    /// 配置：`etc/<编号>/`。
    pub config: PathBuf,
    /// 状态：`var/lib/<编号>/`（扩展的工作目录）。
    pub state: PathBuf,
    /// 缓存：`var/cache/<编号>/`。
    pub cache: PathBuf,
    /// 日志：`var/log/<编号>/`。
    pub log: PathBuf,
    /// 短命暂存：`tmp/<编号>/`。
    pub tmp: PathBuf,
    /// 运行时：`run/<编号>/`。
    pub runtime: PathBuf,
    /// 照 XDG 给你的几处：交出去的名字和位置。
    pub user: Vec<(Names, PathBuf)>,
}

impl PackageDirs {
    /// 数据根 `root` 里包 `id` 的几处，包的文件夹是 `app`。不建。
    pub fn of(root: &DataRoot, id: &str, app: PathBuf) -> PackageDirs {
        PackageDirs {
            app,
            config: root.package_config(id),
            state: root.package_state(id),
            cache: root.package_cache(id),
            log: root.package_logs(id),
            tmp: root.package_tmp(id),
            runtime: root.package_runtime(id),
            user: USER_DIRS
                .iter()
                .filter_map(|name| Some((user_dir(name)?, root.user_dir(name))))
                .collect(),
        }
    }

    /// 拉起之前（设计 55 第四节：程序拿到的一定已经存在）：先清掉上一回留下的短命暂存（[`DataRoot::clear_package_tmp`]：
    /// 路上是链接的不清、不拉起），各处没有的建上（只在数据根里建），交回换成真实位置的那一份。包的文件夹照给的交。
    ///
    /// # Errors
    ///
    /// 短命暂存清不掉、路上是链接；哪一处建不成、换不成真实位置。
    pub fn ready(root: &DataRoot, id: &str, app: PathBuf) -> io::Result<PackageDirs> {
        root.clear_package_tmp(id)?;
        let dirs = PackageDirs::of(root, id, app);
        let real = |dir: &Path| -> io::Result<PathBuf> {
            crate::root::create_inside(dir)?;
            std::fs::canonicalize(dir)
        };
        let user = dirs
            .user
            .iter()
            .map(|(names, dir)| Ok((*names, real(dir)?)))
            .collect::<io::Result<Vec<_>>>()?;
        Ok(PackageDirs {
            config: real(&dirs.config)?,
            state: real(&dirs.state)?,
            cache: real(&dirs.cache)?,
            log: real(&dirs.log)?,
            tmp: real(&dirs.tmp)?,
            runtime: real(&dirs.runtime)?,
            user,
            app: dirs.app,
        })
    }

    /// 交给它的环境变量：每一处照它的名字表，新名字、旧名字都指过去。
    pub fn env(&self) -> Vec<(&'static str, PathBuf)> {
        let mut env: Vec<(&'static str, PathBuf)> = named(APP, &self.app)
            .chain(named(CONFIG, &self.config))
            .chain(named(PACKAGE_STATE, &self.state))
            .chain(named(CACHE, &self.cache))
            .chain(named(LOG, &self.log))
            .chain(named(TMP, &self.tmp))
            .chain(named(RUNTIME, &self.runtime))
            .collect();
        for (names, dir) in &self.user {
            env.extend(named(names, dir));
        }
        env
    }
}

#[cfg(test)]
mod tests;
