//! `package.file`（施工 F-6 中，`package-pages.md`「`package.file`」）：读一个包的后台页目录里的一份文件，一次最多 512 KiB。
//! 吉祥物包（施工 F-7）只给 `[mascot] model` 指的那一份；外观包（施工 F-16、TH-1）给终端主题文件那一份，和网页主题目录下的
//! 任何一份普通文件，路径照包目录写。
//! 媒体类型不给：网页软件照扩展名查它自己的那张表（同它给自己页面的那一套），核心不另放一份。

use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::package::Page;

use crate::Core;
use crate::refusal::Refusal;

/// `package.file` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FileParams {
    package: String,
    path: String,
    #[serde(default)]
    offset: u64,
}

/// 读一块：`{"data", "size", "eof"}`。
pub(crate) async fn read(core: &Core, params: FileParams) -> Result<Value, Refusal> {
    let Located {
        dir,
        segments,
        within,
    } = located(core, &params.package, &params.path)?;
    let offset = params.offset;
    tokio::task::spawn_blocking(move || {
        let real = beneath(&dir, &segments)
            .filter(|real| within.as_deref().is_none_or(|within| inside(real, within)))
            .ok_or(Refusal::FILE_NOT_FOUND)?;
        let segment = miyu_fs::read_range(&real, offset, miyu_fs::MAX_LENGTH)
            .map_err(|_| Refusal::FILE_NOT_FOUND)?;
        let eof = offset.saturating_add(segment.data.len() as u64) >= segment.size;
        Ok(json!({"data": STANDARD.encode(&segment.data), "size": segment.size, "eof": eof}))
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?
}

/// 要读的那一份在哪：`dir` 下照 `segments` 一段段找，换成真实的位置以后还要在 `dir` 里；有 `within` 的还要在它里面。
struct Located {
    dir: PathBuf,
    segments: Vec<String>,
    /// 外观包的网页主题目录（施工 TH-1）：路径照包目录写，`dir` 是包目录，读到的还得在网页目录里，链接指到包里别处的不给。
    within: Option<PathBuf>,
}

impl Located {
    /// 在 `dir` 下找，别的不管。
    fn under(dir: PathBuf, segments: Vec<String>) -> Located {
        Located {
            dir,
            segments,
            within: None,
        }
    }
}

/// 包 `id` 里 `path` 那一份在哪：后台页照路径在页目录下找；吉祥物包只给 `[mascot] model` 那一份（施工 F-7）；外观包给终端
/// 主题文件那一份、网页主题目录下的（施工 TH-1），别的路径 `file_not_found`。没有这个包、没装的 `unknown_package`；都没有的
/// `no_page`；路径写错的 `bad_params`。
fn located(core: &Core, id: &str, path: &str) -> Result<Located, Refusal> {
    let segments = segments(path).ok_or(Refusal::BAD_PARAMS)?;
    let packages = core.system_packages();
    let found = packages
        .iter()
        .find(|found| found.id == id)
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    let manifest = found.read.as_ref().map_err(|_| Refusal::UNKNOWN_PACKAGE)?;
    if let Some(mascot) = &manifest.mascot {
        return match path == mascot.model {
            true => Ok(Located::under(found.files_dir(), segments)),
            false => Err(Refusal::FILE_NOT_FOUND),
        };
    }
    if let Some(theme) = &manifest.theme {
        if theme.tui.as_deref() == Some(path) {
            return Ok(Located::under(found.files_dir(), segments));
        }
        let web = theme
            .web
            .as_deref()
            .filter(|web| under_web(&segments, web))
            .ok_or(Refusal::FILE_NOT_FOUND)?;
        return Ok(Located {
            dir: found.files_dir(),
            segments,
            within: Some(found.files_dir().join(web)),
        });
    }
    match manifest.page.as_ref().and_then(Page::dir) {
        Some(dir) if crate::packages::status::page(found, manifest) => {
            Ok(Located::under(found.files_dir().join(dir), segments))
        }
        _ => Err(Refusal::NO_PAGE),
    }
}

/// 相对路径拆成一段段：空的是 `index.html`；有空段、`.`、`..`、`\`、`:`、控制字符的不收。
pub(crate) fn segments(path: &str) -> Option<Vec<String>> {
    if path.is_empty() {
        return Some(vec!["index.html".to_string()]);
    }
    path.split('/')
        .map(|segment| {
            let bad = segment.is_empty()
                || segment == "."
                || segment == ".."
                || segment.contains(['\\', ':'])
                || segment.chars().any(char::is_control);
            (!bad).then(|| segment.to_string())
        })
        .collect()
}

/// 照包目录写的 `segments` 在网页主题目录 `web` 下面：目录本身不算，要它下面的一份。
fn under_web(segments: &[String], web: &str) -> bool {
    let prefix: Vec<&str> = web.split('/').collect();
    segments.len() > prefix.len()
        && segments
            .iter()
            .zip(&prefix)
            .all(|(segment, part)| segment == part)
}

/// 真实的位置 `real` 在目录 `dir` 换成真实的位置以后的里面。
fn inside(real: &Path, dir: &Path) -> bool {
    std::fs::canonicalize(dir).is_ok_and(|dir| real.starts_with(dir))
}

/// `dir` 下的这一份：换成真实的位置以后还在 `dir` 里、是普通文件的才算（链接指出去的不算）。
pub(crate) fn beneath(dir: &Path, segments: &[String]) -> Option<PathBuf> {
    let root = std::fs::canonicalize(dir).ok()?;
    let real = std::fs::canonicalize(
        segments
            .iter()
            .fold(dir.to_path_buf(), |path, segment| path.join(segment)),
    )
    .ok()?;
    (real.starts_with(&root) && real.is_file()).then_some(real)
}

#[cfg(test)]
mod tests;
