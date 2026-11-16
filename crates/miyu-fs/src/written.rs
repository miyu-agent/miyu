//! 人写的一个目录（施工 5-15 下起，PV-RM 从去掉的私密目录挪来）：配置里写目录的几项（别的技能目录 `skills.external.dirs`）照它
//! 换成路径。

use std::path::{Path, PathBuf};

/// 人写的一个目录：`~`、`~/…`（Windows 上 `~\…` 也算，照 [`crate::tilde`]，施工 WN-1）照家目录拼（一段一段接上），绝对路径
/// 照原样；别的写法没有。
pub fn written(home: &Path, text: &str) -> Option<PathBuf> {
    if let Some(rest) = crate::tilde(text) {
        let joined = rest
            .split(|c| c == '/' || (cfg!(windows) && c == '\\'))
            .filter(|part| !part.is_empty())
            .fold(home.to_path_buf(), |dir, part| dir.join(part));
        return Some(joined);
    }
    let path = Path::new(text);
    path.is_absolute().then(|| path.to_path_buf())
}

#[cfg(test)]
mod tests;
