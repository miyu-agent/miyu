//! 边界表（`11-权限与沙盒.md` 第四节，施工 4-3 上）：一个真实的位置落在哪一片。
//!
//! 几片重叠时最深的那一片算（施工 5-14 改）：数据根里单独放开只读的几个目录（开着的技能文件夹，施工 X-10 上）、数据根、别的
//! 程序的私密目录（施工 5-13）包在工作区、加进来的目录（照工作区算，施工 5-10 上）里面的照它们自己那一片；工作区落在数据根里、
//! 工作区和加进来的目录落在私密目录里的照工作区。再往后是临时目录、系统和工具链目录、边界以外。能写的
//! 那几片（工作区、加进来的目录、临时目录）里落在只禁写名单上的只能读（施工 5-17 下，[`Readonly`]）。

use std::path::{Component, Path, PathBuf};

use crate::Readonly;

/// 一个真实的位置落在哪一片。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    /// 能读能写：工作区、加进来的目录、临时目录。
    Writable,
    /// 只能读：系统目录、工具链目录，能写的那几片里落在只禁写名单上的（工作区里的 `.git/hooks`、`.git/config` 这些）。
    Readable,
    /// 谁都不能碰：Miyu 的数据根。
    Forbidden,
    /// 别的程序的私密目录（施工 5-13，`11-权限与沙盒.md` 第四节）：钥匙、登录、浏览器配置、旧版 Miyu 的数据。文件工具读写都
    /// 问人，完全放开不问；往下走目录的工具不进。
    Private,
    /// 数据根里单独放开的（施工 X-10 上，开着的技能文件夹）：能读，写不行，问人也不行。
    Shelf,
    /// 边界以外：要碰就得问人。
    Outside,
}

/// 造边界表要的几个地方。都是原样的路径，[`Boundary::new`] 再换成真实的位置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// 这一轮的工作区。给之前先照 `11-权限与沙盒.md` 第四节挑过：太宽的已经退回了账号的工作区。
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
    /// 别的程序的私密目录（施工 5-13，[`crate::private_dirs`]）。
    pub private: Vec<PathBuf>,
    /// 只禁写的名单（施工 5-17 下）。
    pub readonly: Readonly,
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
    /// 照这台机器：临时目录、系统目录、家目录下的工具链目录，`CARGO_HOME`、`RUSTUP_HOME` 指到的地方，出厂的私密目录、只禁写
    /// 名单。家目录读不出来的，工具链目录只算环境变量指到的。
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
            private: home.map(crate::private_dirs).unwrap_or_default(),
            readonly: Readonly::shipped(home),
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

/// 边界表：每一片都换成了真实的位置；不存在的那一片不算。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Boundary {
    workspace: Option<PathBuf>,
    dirs: Vec<PathBuf>,
    data_root: Option<PathBuf>,
    temp: Option<PathBuf>,
    readable: Vec<PathBuf>,
    shelves: Vec<PathBuf>,
    private: Vec<PathBuf>,
    /// 只禁写名单里相对的那几项，每一段照 [`fold`] 写。
    inside: Vec<Vec<String>>,
    /// 只禁写名单里家目录下的、绝对路径的，换成真实的位置：还不存在的照最近的、已经存在的上级换（[`crate::resolve()`]）。
    readonly: Vec<PathBuf>,
    /// 这个人自己的几处（施工 Y-11 上），换法同 `readonly`：还没建的照最近的、已经存在的上级换。
    own: Vec<PathBuf>,
}

impl Boundary {
    /// 照 `places` 造：每一片都换成真实的位置，不存在的（例如这台机器上没有 `/lib32`）不算。
    pub fn new(places: &Places) -> Boundary {
        Boundary {
            workspace: real(&places.workspace),
            dirs: places.dirs.iter().filter_map(|dir| real(dir)).collect(),
            data_root: real(&places.data_root),
            temp: real(&places.temp),
            readable: places.readable.iter().filter_map(|dir| real(dir)).collect(),
            shelves: places.shelves.iter().filter_map(|dir| real(dir)).collect(),
            private: places.private.iter().filter_map(|dir| real(dir)).collect(),
            inside: places
                .readonly
                .inside
                .iter()
                .filter(|names| !names.is_empty())
                .map(|names| names.iter().map(|name| fold(name)).collect())
                .collect(),
            readonly: places
                .readonly
                .paths
                .iter()
                .filter_map(|path| crate::resolve(path, None, &path.to_string_lossy()).ok())
                .collect(),
            own: places
                .own
                .iter()
                .filter_map(|path| crate::resolve(path, None, &path.to_string_lossy()).ok())
                .collect(),
        }
    }

    /// 真实的位置 `path` 落在哪一片：先经 [`crate::resolve()`] 换过再交进来。
    pub fn zone(&self, path: &Path) -> Zone {
        self.zone_with(path, true)
    }

    /// 真实的位置 `path` 是不是受保护的路径（施工 5-17 下）：本来能写，落在只禁写名单上才只能读。放行规则照它挑：受保护的只认
    /// 放在受保护的路径里面的规则（`session/guard.md`，同私密目录）。
    pub fn protected(&self, path: &Path) -> bool {
        self.zone_with(path, true) == Zone::Readable
            && self.zone_with(path, false) == Zone::Writable
    }

    /// 同 [`Boundary::zone`]；`listed` 是假的不看只禁写名单。
    ///
    /// 最深的那一片算（施工 5-14 改）：工作区可以是 `~` 这种包住数据根、私密目录的，它们照旧是自己那一片；工作区、加进来的目录
    /// 落在它们里面的（会话的默认工作区在数据根里、人把工作区开在 `~/.config` 里）才照工作区算。
    fn zone_with(&self, path: &Path, listed: bool) -> Zone {
        if self.shelves.iter().any(|dir| within(path, dir)) {
            return Zone::Shelf;
        }
        let workspace = self
            .workspace
            .as_deref()
            .and_then(|root| Some((root, path.strip_prefix(root).ok()?)));
        // 这个人自己的几处（施工 Y-11 上）：在数据根里也照工作区算。
        let own = self
            .own
            .iter()
            .find_map(|root| Some((root.as_path(), path.strip_prefix(root).ok()?)));
        // 数据根：只有工作区、自己的几处落在数据根里时那一片照工作区算；加进来的目录落进了数据根的（报来以后被换成了链接）照样
        // 谁都不能碰。
        if let Some(data_root) = &self.data_root
            && within(path, data_root)
            && !workspace.is_some_and(|(root, _)| within(root, data_root))
            && own.is_none()
        {
            return Zone::Forbidden;
        }
        let held = workspace.or(own).or_else(|| {
            self.dirs
                .iter()
                .find_map(|root| Some((root.as_path(), path.strip_prefix(root).ok()?)))
        });
        // 私密目录：工作区、加进来的目录落在它里面的，照人给的算（施工 5-13）；包住它的不算。
        if self
            .private
            .iter()
            .any(|dir| within(path, dir) && !held.is_some_and(|(root, _)| within(root, dir)))
        {
            return Zone::Private;
        }
        if let Some((_, inside)) = held {
            return self.writable(path, Some(inside), listed);
        }
        if self
            .temp
            .as_ref()
            .is_some_and(|temp| path.starts_with(temp))
        {
            return self.writable(path, None, listed);
        }
        if self.readable.iter().any(|dir| path.starts_with(dir)) {
            return Zone::Readable;
        }
        Zone::Outside
    }

    /// 走目录的工具（`fs.find`，施工 W-2）要不要挡住往下走进 `dir`：`dir` 落进「谁都不能碰」那一片、又没有
    /// 工作区、加进来的目录藏在它底下——工作区常常就在数据根里面（默认的会话就在那里干活），得穿过数据根才能
    /// 走到它，但数据根自己、旁的子目录不许进（施工 W-2）。
    #[must_use]
    pub fn blocks_descent(&self, dir: &Path) -> bool {
        if self.zone(dir) != Zone::Forbidden {
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

impl Boundary {
    /// 能写的那几片里的 `path`（在工作区、加进来的目录里的，那一段是 `inside`）：落在只禁写名单上的只能读（施工 5-17 下）；
    /// `listed` 是假的不看名单。
    fn writable(&self, path: &Path, inside: Option<&Path>, listed: bool) -> Zone {
        let protected = listed
            && (self.readonly.iter().any(|item| within(path, item))
                || inside.is_some_and(|inside| self.listed_inside(path, inside)));
        if protected {
            Zone::Readable
        } else {
            Zone::Writable
        }
    }

    /// 工作区、加进来的目录里的这一段 `inside` 在不在名单上：哪一层接得上相对的那一项都算（子仓库里的 `.git/hooks` 也算）。大小写
    /// 不分的文件系统上，`.GIT/HOOKS` 也是同一处，照不分大小写比。`.git/config` 在名单上的，作为文件的 `.git`（worktree、子模块）
    /// 也算：它说仓库在哪，钩子、配置就在哪。
    fn listed_inside(&self, path: &Path, inside: &Path) -> bool {
        let names: Vec<String> = inside
            .components()
            .filter_map(|component| match component {
                Component::Normal(name) => Some(fold(&name.to_string_lossy())),
                _ => None,
            })
            .collect();
        if self.inside.iter().any(|item| {
            names
                .windows(item.len())
                .any(|window| window == item.as_slice())
        }) {
            return true;
        }
        names.last().is_some_and(|last| last == ".git")
            && self.inside.iter().any(|item| *item == [".git", "config"])
            && !path.is_dir()
    }
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
