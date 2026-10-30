//! 找文件、搜内容往下走目录（`10-自带软件.md` 第三节「`glob`、`grep` 输出的写法」，施工 4-4 下）。
//!
//! - 遵守 `.gitignore`、`.ignore`。在 git 仓库里时，仓库根到搜的那个目录之间的也算；不在仓库里时，只认搜的那个
//!   目录往下的：家目录里要是有一份写着 `*` 的 `.gitignore`（拿 git 管 dotfiles 的常这么配），往上找就什么都搜
//!   不到了。pi 也是这样分的。
//! - 隐藏文件照找；版本库自己的目录（[`VCS`]）不进；不跟着链接走。
//! - Miyu 的数据根不进（[`Fence`]）：权限策略只核对搜的那个目录，从上面搜下来会走进去（`11-权限与沙盒.md` A9）。
//!   这一轮的工作目录在数据根里面的（工作区 `workspace/` 就在数据根里），照样进，和权限策略的先后一样；工作目录包住数据根的
//!   （`~`，施工 5-14 改起它也是工作区）照样不进：最深的那一片算。
//! - 别的程序的私密目录也不进（施工 5-13，`miyu_fs::private_dirs`）：读它们要问人，从上面搜下来不该绕过去。搜的那个
//!   目录就在里面的（问过了）、工作目录那一片照样进。
//! - 按修改时间排，新的在前，时间一样的照路径排。
//! - 不先把整棵树记下来（施工 V-21）：[`each`] 边走边交，要的一方只留自己要的（`glob` 留最新的 [`newest`] 几个、`grep`
//!   留有匹配的）。2026-10-26 从家目录搜下来，先记下的几百万个文件让核心涨到约 900 MB。

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::path::{Component, MAIN_SEPARATOR, Path, PathBuf};
use std::time::SystemTime;

use ignore::WalkBuilder;

use miyu_fs::{private_dirs, resolve, within};
use miyu_tool::{Call, Stop};

/// 版本库自己的目录，不进（Claude Code 的名单）。
const VCS: [&str; 6] = [".git", ".svn", ".hg", ".bzr", ".jj", ".sl"];

/// 走到的一个文件。照交出去的先后比（[`Ord`]）：新的在前，时间一样的照相对路径。
#[derive(Debug)]
pub(crate) struct Found {
    /// 真实的位置。
    pub(crate) path: PathBuf,
    /// 相对搜的那个目录的路径，分隔符一律写 `/`：照它比模式。
    pub(crate) relative: String,
    /// 修改时间：读不出来的当最早。
    modified: SystemTime,
}

impl Ord for Found {
    /// 小的在前：新的在前，时间一样的照相对路径。
    fn cmp(&self, other: &Found) -> Ordering {
        other
            .modified
            .cmp(&self.modified)
            .then_with(|| self.relative.cmp(&other.relative))
    }
}

impl PartialOrd for Found {
    fn partial_cmp(&self, other: &Found) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Found {
    fn eq(&self, other: &Found) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Found {}

/// 走目录时不进的地方：Miyu 的数据根、别的程序的私密目录（施工 5-13），除了这一轮的工作目录那一片。
pub(crate) struct Fence {
    data_root: Option<PathBuf>,
    workspace: Option<PathBuf>,
    /// 私密目录：真实的位置，这台机器上有的。
    private: Vec<PathBuf>,
}

impl Fence {
    /// 照调用 `call`：数据根和工作目录都换成真实的位置，换不成的不算。
    pub(crate) fn of(call: &Call) -> Fence {
        Fence {
            data_root: call
                .data_root
                .as_deref()
                .and_then(|dir| std::fs::canonicalize(dir).ok()),
            workspace: resolve(Path::new(&call.cwd), call.home.as_deref(), &call.cwd).ok(),
            // 照这一轮的配置（施工 5-15 下）：人加的、去掉的；没有配置端口的照出厂的。
            private: call
                .home
                .as_deref()
                .map(|home| match &call.settings {
                    Some(settings) => miyu_tool::private::dirs(&settings.values(), home),
                    None => private_dirs(home),
                })
                .unwrap_or_default()
                .iter()
                .filter_map(|dir| std::fs::canonicalize(dir).ok())
                .collect(),
        }
    }

    /// 从 `root` 往下走：搜的那个目录就在里面的私密目录不挡（问过了才走得到这里）。
    ///
    /// 这棵树里碰不到的也去掉（施工 V-31）：走到的目录都是 `root` 再往下接几段（`ignore` 照上一层一段段 `join` 出来），
    /// [`within`] 又是一段一段比的，`within(dir, fenced)` 成立只有两种：`fenced` 包住 `root`（含等于），或者 `fenced` 落在
    /// `root` 下面。两样都不是的，[`Fence::keeps_out`] 对这棵树里每个目录都答「不挡它」，留着只是每进一个目录白比一遍
    /// （V-29 量过，约占 `grep` 的 23%）。判断用的还是同一个 [`within`]，大小写、前缀的口径和挡的时候一样。
    fn from(mut self, root: &Path) -> Fence {
        let reachable = |fenced: &Path| within(root, fenced) || within(fenced, root);
        self.private
            .retain(|dir| !within(root, dir) && reachable(dir));
        self.data_root = self.data_root.filter(|dir| reachable(dir));
        self
    }

    /// 目录 `dir` 不进：在数据根、私密目录里，除非工作目录就落在那一处里面、`dir` 又在工作目录那一片里（最深的那一片算，施工
    /// 5-14 改：工作目录是 `~` 这种包住数据根、私密目录的，照样不进它们）。
    fn keeps_out(&self, dir: &Path) -> bool {
        self.data_root
            .iter()
            .chain(&self.private)
            .any(|fenced| within(dir, fenced) && !self.held(dir, fenced))
    }

    /// `dir` 在工作目录那一片里，工作目录又落在 `fenced` 里面（工作区 `workspace/` 就在数据根里）。
    fn held(&self, dir: &Path, fenced: &Path) -> bool {
        self.workspace
            .as_deref()
            .is_some_and(|workspace| within(workspace, fenced) && within(dir, workspace))
    }
}

/// 在目录 `root`（已经换成真实的位置）下面走一遍，交回 `keep` 留下的每个普通文件，按修改时间新的在前。
/// `keep` 拿到的是相对 `root`、分隔符写 `/` 的路径。`fence` 挡着的目录不进；叫停了（`stop`）就不往下走了。
/// 整棵树都记下来：只有要照先后一个个搜、够了就停的（`grep` 列匹配的行）用它。
pub(crate) fn files(
    root: &Path,
    fence: Fence,
    stop: &Stop,
    keep: impl Fn(&str) -> bool,
) -> Vec<Found> {
    let mut found = Vec::new();
    each(root, fence, stop, keep, |file| found.push(file));
    found.sort();
    found
}

/// 同 [`files`]，只留最新的 `limit` 个（照先后排好），另交回一共走到几个：不记下整棵树。
pub(crate) fn newest(
    root: &Path,
    fence: Fence,
    stop: &Stop,
    keep: impl Fn(&str) -> bool,
    limit: usize,
) -> (Vec<Found>, usize) {
    // 堆顶是留下的里面最旧的：多出一个就把它挤掉。
    let mut kept = BinaryHeap::with_capacity(limit.saturating_add(1));
    let mut total = 0;
    each(root, fence, stop, keep, |file| {
        total += 1;
        kept.push(file);
        if kept.len() > limit {
            drop(kept.pop());
        }
    });
    (kept.into_sorted_vec(), total)
}

/// 同 [`files`]，只是不排、不记：走到一个交一个给 `found`，照走到的先后。
pub(crate) fn each(
    root: &Path,
    fence: Fence,
    stop: &Stop,
    keep: impl Fn(&str) -> bool,
    mut found: impl FnMut(Found),
) {
    let in_repo = root.ancestors().any(|dir| dir.join(".git").exists());
    let fence = fence.from(root);
    let quick = quick_root(root);
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .follow_links(false)
        .parents(in_repo)
        .require_git(in_repo)
        .filter_entry(move |entry| {
            let dir = entry.file_type().is_some_and(|kind| kind.is_dir());
            if !dir || entry.depth() == 0 {
                return true;
            }
            let vcs = VCS.contains(&entry.file_name().to_string_lossy().as_ref());
            !vcs && !fence.keeps_out(entry.path())
        });
    builder
        .build()
        .take_while(|_| !stop.stopped())
        // 读不了的目录（没有权限这类）跳过，别的照走：ripgrep 也是这样。
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
        .filter_map(|entry| {
            let relative = relative(root, quick, entry.path())?;
            if !keep(&relative) {
                return None;
            }
            let modified = entry
                .metadata()
                .and_then(|meta| meta.modified().map_err(ignore::Error::from))
                .unwrap_or(SystemTime::UNIX_EPOCH);
            Some(Found {
                path: entry.into_path(),
                relative,
                modified,
            })
        })
        .for_each(&mut found);
}

/// 照字面截相对路径用的 `root`（[`quick`]）：写得成 UTF-8、最后一段是名字或者根的才行，别的（例如 Windows 的 `C:`、
/// `\\?\C:` 后面没有 `\`：再往下接一段，多出来的是根，不是名字）交给 [`slow`]。走一遍算一次。
fn quick_root(root: &Path) -> Option<&str> {
    let last = root.components().next_back();
    root.to_str()
        .filter(|_| matches!(last, Some(Component::Normal(_) | Component::RootDir)))
}

/// `path` 相对 `root` 的路径，一段一段用 `/` 连起来。`quick` 是 [`quick_root`] 交回的：有的先照字面截（施工 V-31），截不了的
/// 照 [`slow`] 一段一段比。两条路交回的一字不差（`walk/tests/relative.rs` 对照）。
fn relative(root: &Path, quick: Option<&str>, path: &Path) -> Option<String> {
    quick
        .zip(path.to_str())
        .and_then(|(root, path)| self::quick(root, path))
        .or_else(|| slow(root, path))
}

/// 照字面截：`path` 的字面以 `root` 开头，后面是「分隔符 名字 分隔符 名字……」（往下走目录时走到的都是这样：一层一层
/// `join` 出来的），每个名字不空、不是 `.`、Windows 上不带 `/`（那里 `/` 也可能是分隔符）。不是这样的交 `None`，照
/// [`slow`] 算。只认平台原生的分隔符：Windows 上换成 `/`。
///
/// 为什么：以前每个文件都 `strip_prefix` 再一段段拼，约占 `grep` 的 14%（V-29）。都是 UTF-8 时，逐段转 UTF-8 和整条转一样，
/// 名字里又没有分隔符，截出来就是一段段拼出来的那一条。
fn quick(root: &str, path: &str) -> Option<String> {
    let rest = path.strip_prefix(root)?;
    let rest = match rest.strip_prefix(MAIN_SEPARATOR) {
        Some(rest) => rest,
        None if rest.is_empty() || root.ends_with(MAIN_SEPARATOR) => rest,
        None => return None,
    };
    let odd = |part: &str| part.is_empty() || part == "." || (cfg!(windows) && part.contains('/'));
    if !rest.is_empty() && rest.split(MAIN_SEPARATOR).any(odd) {
        return None;
    }
    Some(if cfg!(windows) {
        rest.replace(MAIN_SEPARATOR, "/")
    } else {
        rest.to_owned()
    })
}

/// 一段一段比：`path` 去掉 `root` 那几段，剩下的每段照不严格的 UTF-8 转成字，用 `/` 连起来。
fn slow(root: &Path, path: &Path) -> Option<String> {
    let rest = path.strip_prefix(root).ok()?;
    let parts: Vec<String> = rest
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}

#[cfg(test)]
mod tests;
