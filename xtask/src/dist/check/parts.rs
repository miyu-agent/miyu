//! 查系统源分包用的三份（施工 SD-2）：核心那一份照仓库比（只有核心的程序、资源、许可证，不带两个头）；头那一份看包文件夹齐不齐、
//! 程序在 `bin/` 能跑、清单的版本和名字对得上；再把头照系统加装层的找法放好，用旁边核心那一份的 `miyu` 看它认不认得是
//! `distro` 层、程序找不找得到（`MIYU_DISTRO_PACKAGES` 指到临时目录，数据根也是临时的）。

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::super::archive::walk;
use super::super::split;
use super::super::{CORE, TUI_PROGRAM, WEB_PROGRAM, read};
use super::{isolated, relative, runnable, unpack};

/// 头的包文件夹编号和它的程序。
pub(super) const HEADS: [(&str, &str); 2] = [("tui", TUI_PROGRAM), ("webui", WEB_PROGRAM)];

/// 照文件名认是哪个头的、什么版本：`<编号>-<版本>-<系统>.tar.gz`。不是的没有。
pub(super) fn head_of(name: &str, target: &str) -> Option<(&'static str, String)> {
    let rest = name.strip_suffix(&format!("-{target}.tar.gz"))?;
    HEADS.iter().find_map(|(id, _)| {
        let version = rest.strip_prefix(&format!("{id}-"))?;
        (!version.is_empty()).then(|| (*id, version.to_string()))
    })
}

/// 核心那一份：三个程序能跑、没有两个头的程序；`LICENSE`、第三方清单和仓库的核心那一份一字不差；`resources/` 照登记的逐项
/// 比字节，多出来的报，两个头的包文件夹不该有。
pub(super) fn core_contents(
    root: &Path,
    dir: &Path,
    resources: &[PathBuf],
) -> Result<Vec<String>, String> {
    let mut problems = Vec::new();
    let exe = std::env::consts::EXE_SUFFIX;
    for program in CORE {
        let file = dir.join(format!("{program}{exe}"));
        if !file.is_file() {
            problems.push(format!("少了程序 {program}{exe}"));
        } else if !runnable(&file) {
            problems.push(format!("{program}{exe} 不能跑（没有执行位）"));
        }
    }
    for (_, program) in HEADS {
        if dir.join(format!("{program}{exe}")).exists() {
            problems.push(format!("核心那一份不该带 {program}{exe}"));
        }
    }
    if fs::read(dir.join("LICENSE")).ok() != fs::read(root.join("LICENSE")).ok() {
        problems.push("LICENSE 和仓库的不一样，或者没有".to_string());
    }
    if fs::read(dir.join("THIRD-PARTY-LICENSES.txt")).ok()
        != fs::read(root.join("THIRD-PARTY-LICENSES.txt")).ok()
    {
        problems.push("THIRD-PARTY-LICENSES.txt 该是仓库里核心那一份".to_string());
    }
    let shipped = dir.join("resources");
    let listed: std::collections::BTreeSet<String> =
        resources.iter().map(|file| relative(file)).collect();
    for inside in &listed {
        match fs::read(shipped.join(inside)) {
            Err(_) => problems.push(format!("少了资源 resources/{inside}")),
            Ok(bytes)
                if fs::read(root.join("resources").join(inside))
                    .ok()
                    .as_deref()
                    != Some(bytes.as_slice()) =>
            {
                problems.push(format!("资源 resources/{inside} 和仓库的不一样"));
            }
            Ok(_) => {}
        }
    }
    let mut entries = Vec::new();
    if shipped.is_dir() {
        walk(&shipped, Path::new(""), &mut entries)?;
    }
    for (inside, source) in &entries {
        let inside = relative(inside);
        if source.is_some() && !listed.contains(&inside) {
            problems.push(format!("resources/{inside} 不在仓库的清单里"));
        }
    }
    for (id, _) in HEADS {
        if shipped.join("packages").join(id).exists() {
            problems.push(format!(
                "核心那一份的 resources/packages/ 不该有 {id}：会和系统加装的那一份重"
            ));
        }
    }
    Ok(problems)
}

/// 头那一份的包文件夹 `dir`（名字就是编号 `id`）：清单在、版本是 `version`；许可证、第三方清单在；程序在 `bin/` 能跑；网页的
/// 另要页面首页。
pub(super) fn head_contents(dir: &Path, id: &str, version: &str) -> Vec<String> {
    let mut problems = Vec::new();
    match read(&dir.join("package.toml")).and_then(|text| split::version_of(&text)) {
        Ok(written) if written == version => {}
        Ok(written) => problems.push(format!("清单写的版本是 {written}，压缩包名是 {version}")),
        Err(why) => problems.push(format!("清单读不成：{why}")),
    }
    for file in ["LICENSE", "THIRD-PARTY-LICENSES.txt"] {
        if !dir.join(file).is_file() {
            problems.push(format!("少了 {file}"));
        }
    }
    let program = HEADS
        .iter()
        .find(|(head, _)| *head == id)
        .map_or("", |(_, program)| *program);
    let exe = std::env::consts::EXE_SUFFIX;
    let file = dir
        .join(miyu_store::packages::BIN)
        .join(format!("{program}{exe}"));
    if !file.is_file() {
        problems.push(format!("少了程序 bin/{program}{exe}"));
    } else if !runnable(&file) {
        problems.push(format!("bin/{program}{exe} 不能跑（没有执行位）"));
    }
    if id == "webui" && !dir.join("pages/index.html").is_file() {
        problems.push("少了 pages/index.html".to_string());
    }
    problems
}

/// 照系统加装层的找法放好再问核心：`core` 是旁边核心那一份的压缩包，`distro` 里已经有解开的头的包文件夹 `id`。核心那一份解到
/// `work`，数据根是 `work/home`，`MIYU_DISTRO_PACKAGES` 指到 `distro`；`miyu pkg list --format json` 里这一项要是 `distro` 层、
/// 不是 `program_missing`、没有问题的代码。跑完叫核心停下。
pub(super) fn as_distro(
    core: &Path,
    work: &Path,
    distro: &Path,
    id: &str,
) -> Result<Vec<String>, String> {
    if !core.is_file() {
        return Ok(vec![format!(
            "旁边没有核心那一份 {}，查不了它在系统加装层认不认得出",
            core.display()
        )]);
    }
    let into = work.join("core");
    unpack(core, &into)?;
    let stem = core
        .file_name()
        .map(|name| {
            name.to_string_lossy()
                .trim_end_matches(".tar.gz")
                .to_string()
        })
        .unwrap_or_default();
    let miyu = into
        .join(&stem)
        .join(format!("miyu{}", std::env::consts::EXE_SUFFIX));
    let home = work.join("home");
    let listed = isolated(&miyu, &home)
        .env(miyu_store::packages::distro::VARIABLE, distro)
        .args(["pkg", "list", "--format", "json"])
        .output();
    let stop = isolated(&miyu, &home).args(["core", "stop"]).status();
    let mut problems = Vec::new();
    match listed {
        Ok(done) if done.status.success() => {
            let items: Value = serde_json::from_slice(&done.stdout).unwrap_or(Value::Null);
            let item = items
                .as_array()
                .into_iter()
                .flatten()
                .find(|item| item["package"] == id);
            match item {
                None => problems.push(format!("核心没列出 {id}")),
                Some(item) => {
                    if item["layer"] != "distro" {
                        problems.push(format!("{id} 该在系统加装层，核心认成了 {}", item["layer"]));
                    }
                    if item["status"] == "program_missing" {
                        problems.push(format!("核心找不到 {id} 的程序"));
                    }
                    if let Some(code) = item["code"].as_str() {
                        problems.push(format!("核心读 {id} 的清单报 {code}"));
                    }
                }
            }
        }
        Ok(done) => problems.push(format!(
            "miyu pkg list 没成功（{}）：{}",
            done.status,
            String::from_utf8_lossy(&done.stderr).trim()
        )),
        Err(error) => problems.push(format!("核心那一份的 miyu 起不来（{error}）")),
    }
    if let Err(error) = stop {
        problems.push(format!("miyu core stop 起不来（{error}）"));
    }
    Ok(problems)
}

#[cfg(test)]
mod tests;
