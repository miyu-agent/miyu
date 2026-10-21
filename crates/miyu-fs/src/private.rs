//! 别的程序的私密目录（施工 5-13，`docs/designs/11-权限与沙盒.md` 第四节）：出厂的清单，加上人在「隐私与安全」里加的、去掉的
//! （施工 5-15 下）。权限策略、沙盒、往下走目录的工具、头浏览文件都照它。
//!
//! 清单写成 `~/…`：同一份既是运行时拼路径的，也是设置页「去掉的出厂项」能选的那几个。

use std::ffi::OsString;
use std::path::{Path, PathBuf};

mod moved;
mod shipped;

pub use shipped::SHIPPED_PRIVATE;

/// 家目录 `home` 下出厂的私密目录：照原样拼，不管在不在，一段一段接上（Windows 上分隔符照它自己的）。不看环境变量。
pub fn private_dirs(home: &Path) -> Vec<PathBuf> {
    private_dirs_with(home, &[], &[], &|_| None)
}

/// 出厂的去掉 `remove` 里写了的（照 `~/…` 的写法比），再加上 `add`（施工 5-15 下）。`add` 里 `~/` 开头的照家目录 `home` 拼，
/// 绝对路径照原样；别的写法、家目录本身和它的上级（[`too_broad`]）不算：手改配置文件写了的，照不写算。重复的只留一个。
/// 留着的出厂项跟着环境变量挪去别处的，新位置接在它后面（施工 5-17 上，`moved.rs`；`env` 照变量名交值）。
pub fn private_dirs_with(
    home: &Path,
    add: &[String],
    remove: &[String],
    env: &dyn Fn(&str) -> Option<OsString>,
) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for shipped in SHIPPED_PRIVATE
        .iter()
        .filter(|shipped| !remove.iter().any(|removed| removed == *shipped))
    {
        for dir in written(home, shipped)
            .into_iter()
            .chain(moved::moved(home, shipped, env))
        {
            if !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
    }
    for added in add {
        if let Some(dir) = written(home, added).filter(|dir| !too_broad(home, dir))
            && !dirs.contains(&dir)
        {
            dirs.push(dir);
        }
    }
    dirs
}

/// 人写的一个目录：`~`、`~/…` 照家目录拼（一段一段接上），绝对路径照原样；别的写法没有。
pub fn written(home: &Path, text: &str) -> Option<PathBuf> {
    if text == "~" {
        return Some(home.to_path_buf());
    }
    if let Some(rest) = text.strip_prefix("~/") {
        let joined = rest
            .split('/')
            .filter(|part| !part.is_empty())
            .fold(home.to_path_buf(), |dir, part| dir.join(part));
        return Some(joined);
    }
    let path = Path::new(text);
    path.is_absolute().then(|| path.to_path_buf())
}

/// 藏了它就什么命令都跑不了（施工 5-15 下，2026-10-20 项目主人定）：家目录本身，或者它的上级（`/`、`/home`）。只照写法比，
/// 不碰磁盘：`..`、链接绕出来的照写的算。
pub fn too_broad(home: &Path, dir: &Path) -> bool {
    let tidy = |path: &Path| -> PathBuf { path.components().collect() };
    let (home, dir) = (tidy(home), tidy(dir));
    home.starts_with(&dir)
}

#[cfg(test)]
mod tests;
