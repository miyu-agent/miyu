//! 换文件：解开新的压缩包、查一遍、和装着的那一份对换。
//!
//! 换法照目录里的每一样逐个改名（不整个目录改名）：Windows 上跑着的 `miyu.exe` 删不掉、但改得了名，整个目录却改不了名。
//! 旧的那几样挪进安装目录里的 `.miyu-old-<旧版本>/`，新的挪进来；中途哪一步出错，照反方向挪回去，留着旧的不动。挪完删
//! `.miyu-old-…`，删不掉的（Windows 上跑着的那个程序）留到下一次 `miyu upgrade` 开头再删（[`tidy`]）。
//! `install.toml` 和这几个暂存目录不动。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::plan::MARKER;

/// 暂存新版的目录（在安装目录里，和它同一块盘，挪的时候只是改名）。
pub(crate) const STAGE: &str = ".miyu-upgrade";

/// 挪出来的旧版放在这个名字开头的目录里。
pub(crate) const OLD: &str = ".miyu-old-";

/// 新版里一定要有的程序（不带 `.exe`）。
const PROGRAMS: [&str; 5] = [
    "miyu",
    "miyu-sandbox",
    "miyu-extract",
    "miyu-tui",
    "miyu-web",
];

/// 解开 `archive` 到 `into`：`.zip` 照 zip，别的照 tar.gz；两种都不许写到目录外面。
pub(crate) fn unpack(archive: &Path, into: &Path) -> Result<(), String> {
    let file = fs::File::open(archive).map_err(|error| error.to_string())?;
    if archive.extension().is_some_and(|ext| ext == "zip") {
        zip::ZipArchive::new(file)
            .and_then(|mut zip| zip.extract(into))
            .map_err(|error| error.to_string())
    } else {
        let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
        tar.set_preserve_permissions(true);
        tar.unpack(into).map_err(|error| error.to_string())
    }
}

/// 新版的目录 `top` 缺了什么：五个程序、`resources/`。都在的是空的。
pub(crate) fn missing(top: &Path, exe: &str) -> Vec<String> {
    let mut missing: Vec<String> = PROGRAMS
        .iter()
        .map(|program| format!("{program}{exe}"))
        .filter(|name| !top.join(name).is_file())
        .collect();
    if !top.join("resources").is_dir() {
        missing.push("resources/".to_string());
    }
    missing
}

/// 安装目录 `dir` 里要换掉的那几样：除了标记文件、暂存和挪出来的旧版。照名字排。
fn current(dir: &Path) -> io::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if name == MARKER || name == STAGE || name.starts_with(OLD) {
            continue;
        }
        names.push(name);
    }
    names.sort();
    Ok(names)
}

/// 换好以后：旧版挪去了哪（删掉了的没有）。
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Swapped {
    /// 删不掉、留着的旧版目录。
    pub(crate) left: Option<PathBuf>,
}

/// 把 `dir` 里的东西换成 `top` 里的：旧的挪进 `old`，新的挪进来，最后删 `old`。`rename` 是挪一样东西（真跑的是
/// [`fs::rename`]，测试换成会在某一步出错的）。出错的照反方向挪回去，交回哪一步、系统的原话。
pub(crate) fn swap(
    dir: &Path,
    top: &Path,
    old: &Path,
    rename: &mut dyn FnMut(&Path, &Path) -> io::Result<()>,
) -> Result<Swapped, (String, String)> {
    let failed = |step: &str, error: io::Error| (step.to_string(), error.to_string());
    let outgoing = current(dir).map_err(|error| failed("read", error))?;
    let mut incoming = Vec::new();
    for entry in fs::read_dir(top).map_err(|error| failed("read", error))? {
        incoming.push(
            entry
                .map_err(|error| failed("read", error))?
                .file_name()
                .to_string_lossy()
                .into_owned(),
        );
    }
    incoming.sort();
    fs::create_dir(old).map_err(|error| failed("move-old", error))?;
    let mut moved_out = Vec::new();
    let mut moved_in = Vec::new();
    let result = (|| {
        for name in &outgoing {
            rename(&dir.join(name), &old.join(name)).map_err(|error| failed("move-old", error))?;
            moved_out.push(name.clone());
        }
        for name in &incoming {
            rename(&top.join(name), &dir.join(name)).map_err(|error| failed("move-new", error))?;
            moved_in.push(name.clone());
        }
        Ok(())
    })();
    if let Err(error) = result {
        // 挪回去：新的退回暂存，旧的放回原处。这一步再出错只能留着，报原来那一步。
        for name in moved_in.iter().rev() {
            if fs::rename(dir.join(name), top.join(name)).is_err() {
                // 挪不回去：留在原处，人看得到。
            }
        }
        for name in moved_out.iter().rev() {
            if fs::rename(old.join(name), dir.join(name)).is_err() {
                // 同上。
            }
        }
        if fs::remove_dir(old).is_err() {
            // 没挪回去的还在里面：留着。
        }
        return Err(error);
    }
    Ok(Swapped {
        left: fs::remove_dir_all(old).err().map(|_| old.to_path_buf()),
    })
}

/// 删掉上一次留下的旧版目录（`.miyu-old-…`）和没删干净的暂存；删不掉的不管。
pub(crate) fn tidy(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let leftover = name.starts_with(OLD) || name == STAGE;
        if leftover && fs::remove_dir_all(entry.path()).is_err() {
            // 还在用（Windows 上跑着的程序）：下次再删。
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
