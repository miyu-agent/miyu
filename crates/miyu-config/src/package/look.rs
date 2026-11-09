//! 清单里给头看的几格（施工 F-6 上，`docs/blueprint/package-pages.md`「清单多的几格」，设计 `30-插件框架.md` 第十三节）：
//! `[package] icon` 是 Lucide 的图标名，`[page] dir` 是软件后台页在包目录里的哪个子目录，`[tray] name` 是托盘菜单里打开后台页
//! 的那一行（施工 TR-2）。只查写法：图标名在 Lucide 里有没有、
//! 目录里有没有 `index.html`，核心不管（头认不出的图标照没写画，没有入口的 `package.list` 不带 `page`）。

use toml_edit::{Item, TableLike};

use super::reader::{Reader, relative_dir};
use super::{Code, PackageKind, Problem};
use crate::phrases::Phrases;

/// 图标名最多几个字符。
const ICON_MAX: usize = 64;

/// 包自带的图标最多几个字节（施工 F-18 上）：256 KiB。核心照它查（不解码、不缩），`miyu pkg pack` 照它缩。
pub const LOGO_MAX_BYTES: u64 = 256 * 1024;

/// 包自带的图标每边最多几个像素（施工 F-18 上）。
pub const LOGO_MAX_SIDE: u32 = 512;

/// `[package] icon`：小写字母开头，只有小写字母、数字、`-`，最多 64 个。
pub(super) fn icon(reader: &Reader<'_>, item: &Item) -> Result<String, Problem> {
    item.as_str()
        .filter(|name| icon_name(name))
        .map(str::to_string)
        .ok_or_else(|| {
            reader.problem(
                Some(item),
                Code::BadIcon,
                "package.icon",
                format!("package.icon must be a Lucide icon name: a lowercase letter, then lowercase letters, digits and -, at most {ICON_MAX}"),
            )
        })
}

/// `[package] logo`（施工 F-18 上）：包目录里的相对路径（写法同 `[page] dir`：不许 `..`、开头的 `/`），扩展名是 `.png` 或
/// `.webp`（不分大小写）。图本身由核心查。
pub(super) fn logo(reader: &Reader<'_>, item: &Item) -> Result<String, Problem> {
    item.as_str()
        .filter(|path| relative_dir(path) && image_name(path))
        .map(str::to_string)
        .ok_or_else(|| {
            reader.problem(
                Some(item),
                Code::BadLogo,
                "package.logo",
                "package.logo must be a relative path to a .png or .webp file inside the package directory".to_string(),
            )
        })
}

/// 文件名是 `.png`、`.webp` 结尾的（不分大小写）。
fn image_name(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".png") || lower.ends_with(".webp")
}

/// 软件后台页从哪来（`[page]`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Page {
    /// 包目录里的子目录（`dir`，施工 F-6 上），入口是里面的 `index.html`：头放在框里显示，没装网页的由核心按需起服务给
    /// （施工 BP-3）。扩展、内置包能写。
    Dir(String),
    /// 包的程序自己起的页面服务（`server = true`，施工 BP-1）：它登记的 `status` 方法回应里的 `web` 是听的端口，头拿登录票拼网址
    /// 开过去。只有扩展能写：要有程序在跑。
    Server,
}

impl Page {
    /// 包目录里的子目录；程序自己起服务的没有。
    pub fn dir(&self) -> Option<&str> {
        match self {
            Page::Dir(dir) => Some(dir),
            Page::Server => None,
        }
    }
}

/// `[page]`：`dir`（包目录里的相对目录，写法同 `[ui] pages_dir`）、`server = true`（施工 BP-1）两样写一样。
pub(super) fn page(
    reader: &Reader<'_>,
    table: &dyn TableLike,
    at: &Item,
    kind: PackageKind,
) -> Result<Page, Problem> {
    reader.only(table, "page", super::fields::PAGE)?;
    let bad = |item: Option<&Item>, why: &str| {
        reader.problem(item, Code::BadPageSource, "page", why.to_string())
    };
    match (table.get("dir"), table.get("server")) {
        (Some(_), Some(server)) => Err(bad(
            Some(server),
            "[page] takes either dir or server, not both",
        )),
        (None, Some(server)) if server.as_bool() != Some(true) => {
            Err(bad(Some(server), "page.server can only be true"))
        }
        (None, Some(server)) if kind != PackageKind::Process => Err(bad(
            Some(server),
            "page.server needs a program that serves the page: only extensions can write it",
        )),
        (None, Some(_)) => Ok(Page::Server),
        _ => {
            let item = reader.required(table, at, "page", "dir")?;
            item.as_str()
                .filter(|dir| relative_dir(dir))
                .map(|dir| Page::Dir(dir.to_string()))
                .ok_or_else(|| {
                    reader.problem(
                        Some(item),
                        Code::BadPageDir,
                        "page.dir",
                        "page.dir must be a relative directory inside the package directory"
                            .to_string(),
                    )
                })
        }
    }
}

/// `[tray]`（施工 TR-2，托盘 `miyu-applet` 的蓝图 `app.md` 第 4 条「软件包挂的入口」）：只有 `name`，必写，分语言。点了打开的总是
/// 这个包自己的后台页，所以不收目标：写了 `name` 以外的格（命令、网址、别的包、别的页）、没写 `[page]`（`page` 是假的）的
/// `bad_tray`，免得托盘成了能跑任意东西的启动器（2026-11-03 项目主人定）。
pub(super) fn tray(
    reader: &Reader<'_>,
    table: &dyn TableLike,
    at: &Item,
    page: bool,
) -> Result<Phrases, Problem> {
    if let Some((key, item)) = table
        .iter()
        .find(|(key, _)| !super::fields::TRAY.contains(key))
    {
        return Err(reader.problem(
            Some(item),
            Code::BadTray,
            &format!("tray.{key}"),
            format!("tray.{key}: a tray entry only opens this package's own [page], so it takes just name"),
        ));
    }
    if !page {
        return Err(reader.problem(
            Some(at),
            Code::BadTray,
            "tray",
            "[tray] opens this package's own [page]: write [page] too".to_string(),
        ));
    }
    let name = reader.required(table, at, "tray", "name")?;
    reader.phrases(name, "tray.name")
}

/// `[mascot]`（施工 F-7）：只有 `model`，必写，包目录里的相对路径（写法同 `[page] dir`：不许 `..`、开头的 `/`）。
pub(super) fn mascot(
    reader: &Reader<'_>,
    table: &dyn TableLike,
    at: &Item,
) -> Result<super::Mascot, Problem> {
    reader.only(table, "mascot", super::fields::MASCOT)?;
    let item = reader.required(table, at, "mascot", "model")?;
    item.as_str()
        .filter(|model| relative_dir(model))
        .map(|model| super::Mascot {
            model: model.to_string(),
        })
        .ok_or_else(|| {
            reader.problem(
                Some(item),
                Code::BadMascotModel,
                "mascot.model",
                "mascot.model must be a relative path inside the package directory".to_string(),
            )
        })
}

/// `[package] protocol`：带程序的必写；只带吉祥物的只有数据、不说协议，写了报 `wrong_kind`（施工 F-7）；`bundle` 同样（施工
/// X-2：带的 MCP 服务说的是 MCP，不是 Miyu 的协议）。
pub(super) fn protocol(
    reader: &Reader<'_>,
    kind: PackageKind,
    table: &dyn TableLike,
    at: &Item,
) -> Result<Option<[u32; 2]>, Problem> {
    if matches!(
        kind,
        PackageKind::Mascot | PackageKind::Theme | PackageKind::Bundle
    ) {
        return match table.get("protocol") {
            None => Ok(None),
            Some(item) => Err(reader.problem(
                Some(item),
                Code::WrongKind,
                "package.protocol",
                "package.protocol is only for a package with a program".to_string(),
            )),
        };
    }
    let item = reader.required(table, at, "package", "protocol")?;
    super::reader::protocol(item).map(Some).ok_or_else(|| {
        reader.problem(
            Some(item),
            Code::BadProtocol,
            "package.protocol",
            "package.protocol must be two non-negative integers [lowest, highest]".to_string(),
        )
    })
}

/// 图标名的写法。
fn icon_name(name: &str) -> bool {
    name.len() <= ICON_MAX
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests;
