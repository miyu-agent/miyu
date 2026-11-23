//! 关起来的会话的沙盒（施工 5-12 下，`11-权限与沙盒.md` 第三节「外部身份」）：写只限这一轮的工作区和加进来的目录；读还加上
//! 数据根，只藏凭据那几处（施工 RD-1，2026-11-23 项目主人定，[`miyu_store::root::DataRoot::secret_places`]）；
//! 程序跑起来必需的系统那一片（施工 MB-1 收窄，[`system_reads`]）、`/dev`、自己的 `/proc/self` 只读；临时目录用工作区里的 `.tmp`（系统的临时目录里有别的程序的文件），家目录照工作目录（施工 MB-1）；
//! 工具链的缓存不给。不看级别：完全放开也照样关进来，只读的照样能读工作区、哪儿都不能写。
//!
//! 不给整个 `/proc`：同一个用户的进程的环境变量都在 `/proc/<编号>/environ`，核心的环境里有各家的 key。只有关得住读的
//! 平台（[`miyu_sandbox::CONFINES_READS`]）才会走到这里：权限策略在别的平台上拒命令。

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use miyu_fs::resolve;
use miyu_kernel::event::Permission;
use miyu_sandbox::{Sandboxed, Spec};
use miyu_store::root::DataRoot;

use super::{Sandbox, real};
use crate::guard::{Effective, effective};

/// 关起来的会话读得到的程序目录（施工 MB-1，2026-10-24 项目主人定：只留程序跑起来必需的系统目录）：放程序和库的几处，整个只读。
/// 不给整个 `/etc`、`/opt`：`/etc` 只给 [`ETC`] 那几样。
#[cfg(target_os = "linux")]
const PROGRAMS: &[&str] = &["/usr", "/bin", "/sbin", "/lib", "/lib32", "/lib64"];

/// macOS：系统、命令行工具、Apple 和第三方的框架、Homebrew（Apple 芯片上在 `/opt/homebrew`，Intel 上在 `/usr/local`，已在
/// `/usr` 里）。不给 `/Applications`、`/Library` 别处、别的 `/opt`。程序起来要读、又不在这些里面的几处（施工 5-12 再补）：
/// `/bin/sh` 照 `/private/var/select/sh` 挑用哪个 shell，时区照 `/etc/localtime` 指到的 `/private/var/db/timezone`，旧版系统的
/// 动态库缓存在 `/private/var/db/dyld`；`xcode-select` 选的开发者目录照 `/private/var/db/xcode_select_link` 指到的（`git` 这类
/// `/usr/bin` 里的是转去那里的壳），换成真实位置时就是它指到的那一处。
#[cfg(target_os = "macos")]
const PROGRAMS: &[&str] = &[
    "/usr",
    "/bin",
    "/sbin",
    "/System",
    "/Library/Developer/CommandLineTools",
    "/Library/Apple",
    "/Library/Frameworks",
    "/opt/homebrew",
    "/private/var/select",
    "/private/var/db/timezone",
    "/private/var/db/dyld",
    "/private/var/db/xcode_select_link",
];

/// 别的平台：关不住读，权限策略先拒了命令，走不到这里。
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
const PROGRAMS: &[&str] = &[];

/// `/etc` 里程序跑起来要的（施工 MB-1）：证书、域名解析、动态库的缓存、时区、查用户名（`whoami`、Python 的 `pwd` 要
/// `passwd`；不给就坏，见施工单 SEC-2）、服务和协议名、Debian 系的命令链接、终端和 readline 的设置、git 的系统设置。不在的跳过。
/// macOS 的 `/etc` 是 `/private/etc`，换成真实位置时自然对上。
#[cfg(unix)]
const ETC: &[&str] = &[
    "ssl",
    "ca-certificates",
    "pki",
    "crypto-policies",
    "resolv.conf",
    "hosts",
    "nsswitch.conf",
    "host.conf",
    "gai.conf",
    "ld.so.cache",
    "ld.so.conf",
    "ld.so.conf.d",
    "localtime",
    "timezone",
    "passwd",
    "group",
    "services",
    "protocols",
    "alternatives",
    "mtab",
    "terminfo",
    "inputrc",
    "gitconfig",
    "mime.types",
];

#[cfg(not(unix))]
const ETC: &[&str] = &[];

/// 关起来的会话读得到的系统那一片（施工 MB-1）：[`PROGRAMS`]，`/etc` 里 [`ETC`] 那几样和 Debian 系 Python 的设置（`/etc/python3.x`，
/// 照这一刻 `/etc` 里以 `python` 开头的），`/dev`、自己的 `/proc/self`。碰磁盘（列 `/etc`）。
fn system_reads() -> Vec<PathBuf> {
    let etc = Path::new("/etc");
    let python = std::fs::read_dir(etc)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("python"))
        .map(|entry| entry.path());
    PROGRAMS
        .iter()
        .map(PathBuf::from)
        .chain(ETC.iter().map(|name| etc.join(name)))
        .chain(python)
        .chain([PathBuf::from("/dev"), PathBuf::from("/proc/self")])
        .collect()
}

impl Sandbox {
    /// 关起来的会话一次调用的沙盒：关到这一轮的工作区、加进来的目录。
    ///
    /// 名单里只放命令改不了的路径（施工 AU-001、AU-002）：助手照路径打开、顺着链接走，名单里一条被命令换成了链接，下一条命令
    /// 就能读写链接指到的地方。工作目录、加进来的目录宽的（[`DefaultWorkspace::broad`](crate::default_workspace::DefaultWorkspace::broad)）
    /// 不给：只准读的名单里一条包住家目录，家目录就整个读得到。数据根整个在名单里，凭据那几处照 `hidden` 挖掉（施工 RD-1）。
    ///
    /// # Errors
    ///
    /// 工作区里的 `.tmp` 建不成；工作目录宽、又没有默认工作区可退，或者默认工作区建不成。
    pub(super) fn confined_call(
        &self,
        permission: &Permission,
        (cwd, dirs): (&str, &[String]),
    ) -> io::Result<Sandboxed> {
        let home = self.home.as_deref();
        let place =
            |dir: &str| resolve(Path::new(dir), home, dir).unwrap_or_else(|_| PathBuf::from(dir));
        let cwd = self.narrowed(place(cwd))?;
        let dirs = dirs
            .iter()
            .map(|dir| place(dir))
            .filter(|dir| !self.broad(dir));
        let mut own: Vec<PathBuf> = std::iter::once(cwd.clone()).chain(dirs).collect();
        own.dedup();
        // `.tmp` 在工作目录里，不单独进名单（施工 AU-001）：命令能把它换成链接。
        let temp = cwd.join(".tmp");
        fresh_dir(&temp)?;
        // 家目录照这一轮的工作目录（施工 MB-1）：不给跑核心的那个用户的家目录，那里读不到（git 这类读 `~/.gitconfig` 的在
        // macOS 上被拒就起不来），名字也透出跑核心的用户（SEC-2）。场所会话的是场所的工作区。
        let home_dir = cwd;
        let mut read = system_reads();
        read.extend(own.iter().cloned());
        // 数据根整个读得到，凭据那几处藏着（施工 RD-1）：别的包存在数据根里的图、文件她要读。
        let root = real(&self.data_root);
        let hidden = DataRoot::at(root.clone()).secret_places().to_vec();
        read.push(root);
        // 开着的技能文件夹只读（施工 X-10 中）：在数据根里，名单里照旧列上（数据根以外的也有）。
        read.extend(self.shelves.iter().map(|dir| real(dir)));
        let write = match effective(permission) {
            Effective::ReadOnly => Vec::new(),
            Effective::Full => own,
        };
        Ok(Sandboxed {
            helper: self.helper.clone(),
            spec: Spec {
                write,
                hidden,
                read: Some(read),
                reveal: Vec::new(),
                sockets: Vec::new(),
            },
            env: vec![
                (OsString::from("TMPDIR"), temp.into_os_string()),
                (OsString::from("HOME"), home_dir.into_os_string()),
            ],
            uncached: false,
        })
    }

    /// 真实位置 `dir` 宽不宽（施工 AU-002）：照默认工作区的算法；没交默认工作区的（测试造的）数据根里的一律算宽。
    fn broad(&self, dir: &Path) -> bool {
        let home = self.home.as_deref().map(real);
        let root = real(&self.data_root);
        match &self.workplace {
            Some(place) => place.broad(dir, home.as_deref(), &root),
            None => miyu_fs::broad(dir, home.as_deref(), &root, &root) || dir.starts_with(&root),
        }
    }

    /// 工作目录 `cwd`（真实位置）宽的退回默认工作区（施工 AU-002），不宽的照原样。
    ///
    /// # Errors
    ///
    /// 宽、又没有默认工作区可退；默认工作区建不成。
    fn narrowed(&self, cwd: PathBuf) -> io::Result<PathBuf> {
        if !self.broad(&cwd) {
            return Ok(cwd);
        }
        match &self.workplace {
            Some(place) => place.made_fallback(),
            None => Err(io::Error::other("working directory too wide")),
        }
    }
}

/// `path` 是一个真的目录（施工 AU-001）：命令换成了链接、文件的先删掉再建，不顺着链接在核心这边建目录。
///
/// # Errors
///
/// 删不掉、建不成。
fn fresh_dir(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => return Ok(()),
        // Windows 上指向目录的链接要按目录删；这里走不到 Windows（关不住读的平台不进这里），照样写全。
        Ok(meta) if meta.file_type().is_symlink() => {
            std::fs::remove_file(path).or_else(|_| std::fs::remove_dir(path))?;
        }
        Ok(_) => std::fs::remove_file(path)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    std::fs::create_dir(path)
}

#[cfg(test)]
mod tests;
