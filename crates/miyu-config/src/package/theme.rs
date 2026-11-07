//! 外观包的 `[theme]`（施工 F-16，TH-1 起分终端、网页两样，`docs/blueprint/packages.md`「外观包」）：`[theme.tui] file` 是终端
//! 主题文件，`[theme.web] dir` 是网页主题目录，至少写一样。以前的写法 `[theme] file` 照认，读成 `[theme.tui] file`：装着的
//! 包不用改。新老写法都写了的，老的 `file` 报 `unknown_key`：免得两份路径不知道认哪个。

use toml_edit::{Item, TableLike};

use super::reader::{Reader, relative_dir};
use super::{Code, Problem, fields};

/// 外观包的 `[theme]`（施工 F-16、TH-1，`packages.md`「外观包」）：终端主题、网页主题至少带一样。里面怎么写由终端、网页各自定，
/// 核心只认它们在哪，同吉祥物。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    /// 终端主题文件（`[theme.tui] file`，以前的写法 `[theme] file` 也读进这里）：包目录里的相对路径，例如 `theme.json`；没带的没有。
    pub tui: Option<String>,
    /// 网页主题目录（`[theme.web] dir`）：包目录里的相对目录，里面是一个普通的网页包（`manifest.json`、`index.js` 等，可以带
    /// 脚本，2026-11-02 项目主人定）；没带的没有。
    pub web: Option<String>,
}

/// 读 `[theme]`：表是 `table`，表头在 `at`。
pub(super) fn read(
    reader: &Reader<'_>,
    table: &dyn TableLike,
    at: &Item,
) -> Result<Theme, Problem> {
    reader.only(table, "theme", fields::THEME)?;
    let tui = match (table.get("tui"), table.get("file")) {
        (Some(_), Some(old)) => {
            return Err(reader.problem(
                Some(old),
                Code::UnknownKey,
                "theme.file",
                "theme.file is the old spelling of theme.tui.file: write only one of them"
                    .to_string(),
            ));
        }
        (Some(item), None) => Some(tui(reader, item)?),
        (None, Some(old)) => Some(path(reader, old, "theme.file", Code::BadThemeFile)?),
        (None, None) => None,
    };
    let web = table.get("web").map(|item| web(reader, item)).transpose()?;
    if tui.is_none() && web.is_none() {
        return Err(reader.problem(
            Some(at),
            Code::MissingKey,
            "theme.tui / theme.web",
            "a theme package carries [theme.tui] or [theme.web]".to_string(),
        ));
    }
    Ok(Theme { tui, web })
}

/// `[theme.tui]`：只有 `file`，必写，包目录里的相对路径（写法同 `[mascot] model`）。
fn tui(reader: &Reader<'_>, item: &Item) -> Result<String, Problem> {
    let table = sub_table(reader, item, "theme.tui", Code::BadThemeFile)?;
    reader.only(table, "theme.tui", fields::THEME_TUI)?;
    let file = reader.required(table, item, "theme.tui", "file")?;
    path(reader, file, "theme.tui.file", Code::BadThemeFile)
}

/// `[theme.web]`：只有 `dir`，必写，包目录里的相对目录（写法同 `[page] dir`）。
fn web(reader: &Reader<'_>, item: &Item) -> Result<String, Problem> {
    let table = sub_table(reader, item, "theme.web", Code::BadThemeDir)?;
    reader.only(table, "theme.web", fields::THEME_WEB)?;
    let dir = reader.required(table, item, "theme.web", "dir")?;
    path(reader, dir, "theme.web.dir", Code::BadThemeDir)
}

/// `tui`、`web` 要写成表：`theme.tui = "x.json"` 这种报 `code`。
fn sub_table<'t>(
    reader: &Reader<'_>,
    item: &'t Item,
    name: &str,
    code: Code,
) -> Result<&'t dyn TableLike, Problem> {
    item.as_table_like().ok_or_else(|| {
        reader.problem(
            Some(item),
            code,
            name,
            format!("{name} must be a table: [{name}]"),
        )
    })
}

/// 包目录里的相对路径：不许 `..`、开头的 `/`，写错报 `code`，`key` 是它在清单里的名字。
fn path(reader: &Reader<'_>, item: &Item, key: &str, code: Code) -> Result<String, Problem> {
    item.as_str()
        .filter(|path| relative_dir(path))
        .map(str::to_string)
        .ok_or_else(|| {
            reader.problem(
                Some(item),
                code,
                key,
                format!("{key} must be a relative path inside the package directory"),
            )
        })
}

#[cfg(test)]
mod tests;
