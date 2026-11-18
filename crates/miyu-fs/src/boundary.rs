//! 边界表（`11-权限与沙盒.md` 第四节，施工 4-3 上）：一个真实的位置落在哪一片。
//!
//! 几片重叠时最深的那一片算（施工 5-14 改）：数据根里单独放开只读的几个目录（开着的技能文件夹，施工 X-10 上）、数据根包在工作区、
//! 加进来的目录（照工作区算，施工 5-10 上）里面的照它们自己那一片；工作区落在数据根里的照工作区。再往后是临时目录、系统和
//! 工具链目录、边界以外。别的程序的私密目录这一片去掉了（施工 PV-RM，2026-11-16 项目主人定：只读就是不让写，不替人藏东西）。

use std::path::{Path, PathBuf};

/// 一个真实的位置落在哪一片。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    /// 能读能写：工作区、加进来的目录、临时目录。
    Writable,
    /// 只能读：系统目录、工具链目录。
    Readable,
    /// Miyu 的数据根：文件工具两种模式都读得到（施工 RO-2），完全放开下也写得进、同别的目录（施工 RO-3，2026-11-17 项目主人
    /// 定）。头列文件、找文件、读文件、附件（协议端点）照旧不给这一片，由端点自己判；只碰得到自己工作区的会话碰不到它。
    DataRoot,
    /// 数据根里单独放开的（施工 X-10 上，开着的技能文件夹）：只碰得到自己工作区的会话也读得到它（写不行），读不到数据根别处。
    /// 本人的会话照级别判，同数据根。
    Shelf,
    /// 边界以外：要碰就得问人。
    Outside,
}

/// 造边界表要的几个地方。都是原样的路径，[`Boundary::new`] 再换成真实的位置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// 这一轮的工作区。给之前先照 `11-权限与沙盒.md` 第四节挑过：太宽的已经退回了数据根的工作区。
    pub workspace: PathBuf,
    /// 这一轮加进来的目录，每一个都照工作区算（施工 5-10 上）。太宽的，协议端点已经拒绝了。
    pub dirs: Vec<PathBuf>,
    /// Miyu 的数据根。
    pub data_root: PathBuf,
    /// 临时目录。
    pub temp: PathBuf,
    /// 只能读的：系统目录、工具链目录。
    pub readable: Vec<PathBuf>,
    /// 数据根里单独放开只读的目录（施工 X-10 上）：开着的技能文件夹。
    pub shelves: Vec<PathBuf>,
    /// 这个人自己的几处（施工 Y-11 上：家目录里照 XDG 的 pictures、videos、documents、downloads）：照工作区算，落在数据根里也是；
    /// 还没建的也算。只给文件工具，不进沙盒的写名单（那是调的一方拼的）。
    pub own: Vec<PathBuf>,
}

/// 系统目录的第一版清单（11 第四节）。
#[cfg(target_os = "linux")]
const SYSTEM: &[&str] = &[
    "/usr", "/bin", "/sbin", "/lib", "/lib32", "/lib64", "/etc", "/opt",
];

/// 系统目录的第一版清单（11 第四节）。`/etc` 的真实位置在 `/private/etc`，换成真实的位置时自然就对上了。
#[cfg(target_os = "macos")]
const SYSTEM: &[&str] = &[
    "/usr",
    "/bin",
    "/sbin",
    "/System",
    "/Library",
    "/Applications",
    "/opt",
    "/etc",
];

/// 别的 Unix：还没有清单，系统目录一个都不开，要读就问人。
#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
const SYSTEM: &[&str] = &[];

/// 家目录下的工具链目录（11 第四节）。
const TOOLCHAINS: &[&str] = &[".cargo", ".rustup", ".npm", ".gitconfig"];

impl Places {
    /// 照这台机器：临时目录、系统目录、家目录下的工具链目录，`CARGO_HOME`、`RUSTUP_HOME` 指到的地方。
    /// 家目录读不出来的，工具链目录只算环境变量指到的。
    pub fn here(workspace: PathBuf, data_root: PathBuf, home: Option<&Path>) -> Places {
        let mut readable = system_dirs();
        if let Some(home) = home {
            readable.extend(TOOLCHAINS.iter().map(|name| home.join(name)));
        }
        for name in ["CARGO_HOME", "RUSTUP_HOME"] {
            if let Some(dir) = std::env::var_os(name).filter(|dir| !dir.is_empty()) {
                readable.push(PathBuf::from(dir));
            }
        }
        Places {
            workspace,
            dirs: Vec::new(),
            data_root,
            temp: std::env::temp_dir(),
            readable,
            shelves: Vec::new(),
            own: Vec::new(),
        }
    }
}

/// 这台机器的系统目录：只能读的那一片（11 第四节）；外部身份的会话的沙盒只准读它们和自己的工作区（施工 5-12 下）。
#[cfg(unix)]
pub fn system_dirs() -> Vec<PathBuf> {
    SYSTEM.iter().map(PathBuf::from).collect()
}

/// 这台机器的系统目录：`%SystemRoot%`、`%ProgramFiles%`、`%ProgramFiles(x86)%`，没设的不算。
#[cfg(windows)]
pub fn system_dirs() -> Vec<PathBuf> {
    ["SystemRoot", "ProgramFiles", "ProgramFiles(x86)"]
        .iter()
        .filter_map(std::env::var_os)
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// 边界表：每一片都换成了真实的位置；不存在的那一片不算，数据根、这个人自己的几处除外（还没建的照最近的、已经存在的上级换）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Boundary {
    workspace: Option<PathBuf>,
    dirs: Vec<PathBuf>,
    data_root: Option<PathBuf>,
    temp: Option<PathBuf>,
    readable: Vec<PathBuf>,
    shelves: Vec<PathBuf>,
    /// 这个人自己的几处（施工 Y-11 上）：还没建的照最近的、已经存在的上级换（[`crate::resolve()`]）。
    own: Vec<PathBuf>,
}

impl Boundary {
    /// 照 `places` 造：每一片都换成真实的位置，不存在的（例如这台机器上没有 `/lib32`）不算。
    ///
    /// 数据根还没建的照样算（施工 SEC-4）：照最近的、已经存在的上级换，同 [`crate::resolve()`] 换目标的办法。放开的那几片
    /// 不存在就不算，漏掉的只是少放开一处；数据根是往回收的那一片，不算它，底下的路径就落到别的片里去了。只 `canonicalize`
    /// 也不够：上级目录里有链接（macOS 的 `/var`）、数据根又还没建时，原样的路径和目标的真实位置对不上。
    pub fn new(places: &Places) -> Boundary {
        Boundary {
            workspace: real(&places.workspace),
            dirs: places.dirs.iter().filter_map(|dir| real(dir)).collect(),
            data_root: resolved(&places.data_root),
            temp: real(&places.temp),
            readable: places.readable.iter().filter_map(|dir| real(dir)).collect(),
            shelves: places.shelves.iter().filter_map(|dir| real(dir)).collect(),
            own: places.own.iter().filter_map(|dir| resolved(dir)).collect(),
        }
    }

    /// 真实的位置 `path` 落在哪一片：先经 [`crate::resolve()`] 换过再交进来。
    ///
    /// 最深的那一片算（施工 5-14 改）：工作区可以是 `~` 这种包住数据根的，数据根照旧是自己那一片；工作区落在数据根里的（会话的
    /// 默认工作区在数据根里）才照工作区算。
    pub fn zone(&self, path: &Path) -> Zone {
        if self.shelves.iter().any(|dir| within(path, dir)) {
            return Zone::Shelf;
        }
        let workspace = self
            .workspace
            .as_deref()
            .filter(|root| path.starts_with(root));
        // 这个人自己的几处（施工 Y-11 上）：在数据根里也照工作区算。
        let own = self.own.iter().any(|root| path.starts_with(root));
        // 数据根：只有工作区、自己的几处本身落在数据根里时那一片照工作区算（自己的几处同工作区，审计 #030 第 1 条，施工 AU-P1：
        // 被换成指到家目录这种包住数据根的链接的，原来连密钥、令牌都算能写）；加进来的目录落进了数据根的（报来以后被换成了链接）
        // 照样写不进。
        if let Some(data_root) = &self.data_root
            && within(path, data_root)
            && !workspace.is_some_and(|root| within(root, data_root))
            && !self
                .own
                .iter()
                .any(|root| path.starts_with(root) && within(root, data_root))
        {
            return Zone::DataRoot;
        }
        if workspace.is_some() || own || self.dirs.iter().any(|root| path.starts_with(root)) {
            return Zone::Writable;
        }
        if self
            .temp
            .as_ref()
            .is_some_and(|temp| path.starts_with(temp))
        {
            return Zone::Writable;
        }
        if self.readable.iter().any(|dir| path.starts_with(dir)) {
            return Zone::Readable;
        }
        Zone::Outside
    }

    /// 走目录的工具（`fs.find`，施工 W-2）要不要挡住往下走进 `dir`：`dir` 落进数据根那一片、又没有
    /// 工作区、加进来的目录藏在它底下——工作区常常就在数据根里面（默认的会话就在那里干活），得穿过数据根才能
    /// 走到它，但数据根自己、旁的子目录不许进（施工 W-2）。
    #[must_use]
    pub fn blocks_descent(&self, dir: &Path) -> bool {
        if self.zone(dir) != Zone::DataRoot {
            return false;
        }
        let leads_somewhere_writable = self
            .workspace
            .as_ref()
            .is_some_and(|workspace| workspace.starts_with(dir))
            || self.dirs.iter().any(|d| d.starts_with(dir))
            || self.own.iter().any(|d| d.starts_with(dir));
        !leads_somewhere_writable
    }
}

/// 换成真实的位置；不存在、读不了的不算。
fn real(path: &Path) -> Option<PathBuf> {
    std::fs::canonicalize(path).ok()
}

/// 换成真实的位置，还没建的照最近的、已经存在的上级换（[`crate::resolve()`]）；换不成的（空的路径、上级里有指向不存在处的
/// 链接）不算。
fn resolved(path: &Path) -> Option<PathBuf> {
    crate::resolve(path, None, &path.to_string_lossy()).ok()
}

/// 真实的位置 `path` 在不在目录 `root` 里（`root` 自己也算）：大小写不分的平台上不分大小写，免得换个大小写就
/// 绕过去。两个都要先换成真实的位置。往下走目录的工具照它跳过 Miyu 的数据根（施工 4-4 下）。
pub fn within(path: &Path, root: &Path) -> bool {
    if !CASE_INSENSITIVE {
        return path.starts_with(root);
    }
    let path: Vec<String> = path
        .components()
        .map(|c| fold(&c.as_os_str().to_string_lossy()))
        .collect();
    let root: Vec<String> = root
        .components()
        .map(|c| fold(&c.as_os_str().to_string_lossy()))
        .collect();
    path.starts_with(&root)
}

/// 这个平台的文件系统通常不分大小写：macOS、Windows。
const CASE_INSENSITIVE: bool = cfg!(any(target_os = "macos", windows));

/// 比较用的写法：大小写不分的平台上换成小写，别的照原样。
fn fold(name: &str) -> String {
    if CASE_INSENSITIVE {
        name.to_lowercase()
    } else {
        name.to_string()
    }
}
