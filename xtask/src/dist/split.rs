//! 系统源分包用的三份（施工 SD-2，设计 `60-系统源分包.md` 第二节、第六节第 2 条）：AUR、Homebrew 照 Release 里这几份装，包管理器
//! 只管把文件放到位。一体的压缩包照旧。
//!
//! - `miyu-core-<核心版本>-<系统>.tar.gz`：一层同名目录，`miyu`、`miyu-sandbox`、`miyu-extract`，`resources/`（只有核心自己登记的，
//!   不带两个头的包文件夹，免得出厂、系统加装两层各有一份），`LICENSE`，`THIRD-PARTY-LICENSES.txt`（只有核心那一段）。AUR 整层放到
//!   `/usr/lib/miyu/`，Homebrew 放 `libexec/`；
//! - `miyu-tui-<终端版本>-<系统>.tar.gz`、`miyu-webui-<网页版本>-<系统>.tar.gz`：里面直接是包文件夹（`miyu-tui/`、`miyu-webui/`），
//!   清单照原样，程序在包文件夹的 `bin/`（系统加装那一层照它找程序），带各自的 `LICENSE`、第三方清单；包管理器原样拷到
//!   `share/miyu/packages/<编号>/`。版本照头自己清单写的。
//!
//! 只在 Linux、macOS 上打：Windows 没有系统源（不做 scoop，core1 11-05 定）。

use std::path::{Path, PathBuf};

use toml_edit::DocumentMut;

use super::layout::{copy, executable, make_dir, write};

/// 这一种系统出不出分包：Windows 不出。
pub(super) fn wanted(target: &str) -> bool {
    !target.starts_with("windows")
}

/// 核心那一份的压缩包名、里面那一层目录的名字（同名，去掉后缀）。
pub(super) fn core_name(version: &str, target: &str) -> String {
    format!("miyu-core-{version}-{target}")
}

/// 一个头那一份的压缩包名（不带后缀）：`<编号>-<版本>-<系统>`。
pub(super) fn head_name(id: &str, version: &str, target: &str) -> String {
    format!("{id}-{version}-{target}")
}

/// 清单 `[package] version`。
pub(super) fn version_of(manifest: &str) -> Result<String, String> {
    let doc: DocumentMut = manifest
        .parse()
        .map_err(|error| format!("package.toml 不是 TOML（{error}）"))?;
    doc.get("package")
        .and_then(|item| item.get("version"))
        .and_then(|item| item.as_str())
        .map(str::to_string)
        .ok_or_else(|| "package.toml 的 [package] 里没有 version".to_string())
}

/// 搭核心那一份：`programs` 是三个程序（完整路径），`resources` 下 `files`（相对它）照拷，`license`、`notices` 原样。`dir` 不能已经有。
pub(super) fn stage_core(
    programs: &[PathBuf],
    (resources, files): (&Path, &[PathBuf]),
    license: &Path,
    notices: &str,
    dir: &Path,
) -> Result<(), String> {
    if dir.exists() {
        return Err(format!("{} 已经有了", dir.display()));
    }
    make_dir(dir)?;
    for program in programs {
        let name = program
            .file_name()
            .ok_or_else(|| format!("{} 不是文件", program.display()))?;
        let to = dir.join(name);
        copy(program, &to)?;
        executable(&to)?;
    }
    for file in files {
        copy(&resources.join(file), &dir.join("resources").join(file))?;
    }
    copy(license, &dir.join("LICENSE"))?;
    write(&dir.join("THIRD-PARTY-LICENSES.txt"), notices)
}

/// 搭终端那一份的包文件夹 `dir`（名字就是编号）：`files` 是源文件和在包文件夹里的位置（清单、许可证），`program` 放进
/// `bin/`、带执行位。`dir` 不能已经有。
pub(super) fn stage_head(
    files: &[(PathBuf, PathBuf)],
    program: &Path,
    dir: &Path,
) -> Result<(), String> {
    if dir.exists() {
        return Err(format!("{} 已经有了", dir.display()));
    }
    make_dir(dir)?;
    for (from, inside) in files {
        copy(from, &dir.join(inside))?;
    }
    let name = program
        .file_name()
        .ok_or_else(|| format!("{} 不是文件", program.display()))?;
    let to = dir.join(miyu_store::packages::BIN).join(name);
    copy(program, &to)?;
    executable(&to)
}

#[cfg(test)]
mod tests;
