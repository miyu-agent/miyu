//! `cargo xtask dist-check <压缩包>`（施工 RL-2，设计 `59-发版.md` 第四节第 4 条、`12-进程形态与分发.md` 第八节 R14）：发布前
//! 查包。解开 `cargo xtask dist` 打的压缩包，一样一样对：
//!
//! 1. 名字是 `miyu-<工作区版本>-<本机>.tar.gz`（Windows `.zip`），里面只有一层同名目录；
//! 2. 五个程序都在（Unix 上能跑）；`LICENSE` 和仓库的一字不差；`THIRD-PARTY-LICENSES.txt` 里有核心那一份；`README.txt` 写的是这一版、
//!    这一种；
//! 3. `resources/` 照仓库登记的清单逐项比字节（旧版漏过资源文件），多出来的也报；两个头的包文件夹齐（终端的清单和许可证，网页的
//!    清单和 `pages/index.html`，网页不带 `bin/`）；
//! 4. 跑包里的 `miyu --version`，报的是这一版；隔离的临时数据根里跑 `miyu check`，要过（不设 `MIYU_RESOURCES`，照旁边的 `resources/`
//!    找），跑完 `miyu core stop`。
//!
//! 有一样不对就退出码 1，这一趟不发。解开的目录查完删掉。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use super::archive::{slashed, walk};
use super::fetch::tracked;
use super::layout::{make_dir, remove};
use super::{CORE, HOST, TUI_PROGRAM, WEB_PROGRAM, archive_name, read, version};

/// `cargo xtask dist-check <压缩包>`。
pub fn run(root: &Path, args: &[String]) -> ExitCode {
    let [archive] = args else {
        eprintln!("用法：cargo xtask dist-check <压缩包>");
        return ExitCode::from(2);
    };
    match check(root, Path::new(archive)) {
        Ok(problems) if problems.is_empty() => {
            println!("查包：没有问题");
            ExitCode::SUCCESS
        }
        Ok(problems) => {
            problems
                .iter()
                .for_each(|problem| println!("  ✗ {problem}"));
            println!("查包：{} 处不对，这一趟不发", problems.len());
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("查包：{error}");
            ExitCode::FAILURE
        }
    }
}

fn check(root: &Path, archive: &Path) -> Result<Vec<String>, String> {
    // 查包的家目录就在压缩包旁边，核心只认绝对路径的 MIYU_HOME。
    let archive = &std::path::absolute(archive)
        .map_err(|error| format!("{} 换不成绝对路径：{error}", archive.display()))?;
    let version = version(&read(&root.join("Cargo.toml"))?)?;
    let target = HOST.ok_or("本机不是发的四种系统之一，跑不了包里的程序")?;
    let name = archive
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let expected = archive_name(&version, target);
    // 系统源分包用的几份（施工 SD-2）：核心那一份、两个头的。
    let core = format!("{}.tar.gz", super::split::core_name(&version, target));
    if super::split::wanted(target) && name == core {
        return checked_core(root, archive, &version, target);
    }
    if super::split::wanted(target)
        && let Some((id, head_version)) = parts::head_of(&name, target)
    {
        return checked_head(archive, &archive.with_file_name(&core), id, &head_version);
    }
    if name != expected {
        return Ok(vec![format!(
            "压缩包叫 {name}，这一版、这一种该叫 {expected}（或者分包的 {core}、miyu-tui-<版本>-{target}.tar.gz、\
             miyu-webui-<版本>-{target}.tar.gz）"
        )]);
    }
    let stem = format!("miyu-{version}-{target}");
    let work = archive.with_file_name(format!("{stem}.check"));
    remove(&work)?;
    make_dir(&work)?;
    let found = unpack(archive, &work).and_then(|()| {
        let mut problems = top(&work, &stem)?;
        if problems.is_empty() {
            let dir = work.join(&stem);
            let resources = tracked(root, "resources")?;
            problems.extend(contents(root, &dir, &version, target, &resources)?);
            problems.extend(runs(&dir, &work.join("home"), &version));
        }
        Ok(problems)
    });
    remove(&work)?;
    found
}

/// 核心那一份（施工 SD-2）：一层 `miyu-core-<版本>-<系统>/`，照仓库比，再跑包里的 `miyu --version`、隔离的数据根里 `miyu check`。
fn checked_core(
    root: &Path,
    archive: &Path,
    version: &str,
    target: &str,
) -> Result<Vec<String>, String> {
    let stem = super::split::core_name(version, target);
    let work = archive.with_file_name(format!("{stem}.check"));
    remove(&work)?;
    make_dir(&work)?;
    let found = unpack(archive, &work).and_then(|()| {
        let mut problems = top(&work, &stem)?;
        if problems.is_empty() {
            let dir = work.join(&stem);
            let resources = tracked(root, "resources")?;
            problems.extend(parts::core_contents(root, &dir, &resources)?);
            problems.extend(runs(&dir, &work.join("home"), version));
        }
        Ok(problems)
    });
    remove(&work)?;
    found
}

/// 头那一份（施工 SD-2）：一层包文件夹 `<编号>/`，看齐不齐；再照系统加装层的找法放好，问旁边核心那一份 `core` 认不认得出。
fn checked_head(
    archive: &Path,
    core: &Path,
    id: &str,
    version: &str,
) -> Result<Vec<String>, String> {
    let work = archive.with_file_name(format!("{id}.check"));
    remove(&work)?;
    make_dir(&work)?;
    let distro = work.join("distro");
    let found = unpack(archive, &distro).and_then(|()| {
        let mut problems = top(&distro, id)?;
        if problems.is_empty() {
            problems.extend(parts::head_contents(&distro.join(id), id, version));
            problems.extend(parts::as_distro(core, &work, &distro, id)?);
        }
        Ok(problems)
    });
    remove(&work)?;
    found
}

/// 解开 `archive` 到 `into`：`.zip` 照 zip，别的照 tar.gz。两种都不许写到目录外面（`..`、绝对路径）。
pub(super) fn unpack(archive: &Path, into: &Path) -> Result<(), String> {
    let file =
        fs::File::open(archive).map_err(|error| format!("{}：{error}", archive.display()))?;
    let failed = |error: &dyn std::fmt::Display| format!("{} 解不开：{error}", archive.display());
    if archive.extension().is_some_and(|ext| ext == "zip") {
        zip::ZipArchive::new(file)
            .and_then(|mut zip| zip.extract(into))
            .map_err(|error| failed(&error))
    } else {
        let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
        tar.set_preserve_permissions(true);
        tar.unpack(into).map_err(|error| failed(&error))
    }
}

/// 解出来的只有一层 `stem/`。
fn top(work: &Path, stem: &str) -> Result<Vec<String>, String> {
    let names: Vec<String> = fs::read_dir(work)
        .map_err(|error| format!("{}：{error}", work.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    Ok(match names.as_slice() {
        [one] if one == stem && work.join(one).is_dir() => Vec::new(),
        _ => vec![format!("里面该只有一层 {stem}/，解出来的是 {names:?}")],
    })
}

/// 照仓库比包里的东西。`resources` 是仓库里登记了的资源文件（相对仓库根，`resources/…`）。
pub(super) fn contents(
    root: &Path,
    dir: &Path,
    version: &str,
    target: &str,
    resources: &[PathBuf],
) -> Result<Vec<String>, String> {
    let mut problems = Vec::new();
    let exe = if target.starts_with("windows") {
        ".exe"
    } else {
        ""
    };
    let programs = CORE.iter().copied().chain([TUI_PROGRAM, WEB_PROGRAM]);
    for program in programs {
        let file = dir.join(format!("{program}{exe}"));
        if !file.is_file() {
            problems.push(format!("少了程序 {program}{exe}"));
        } else if !runnable(&file) {
            problems.push(format!("{program}{exe} 不能跑（没有执行位）"));
        }
    }
    if fs::read(dir.join("LICENSE")).ok() != fs::read(root.join("LICENSE")).ok() {
        problems.push("LICENSE 和仓库的不一样，或者没有".to_string());
    }
    let core_notices = read(&root.join("THIRD-PARTY-LICENSES.txt"))?;
    let notices = fs::read_to_string(dir.join("THIRD-PARTY-LICENSES.txt")).unwrap_or_default();
    if !notices.contains(core_notices.trim_end()) {
        problems.push("THIRD-PARTY-LICENSES.txt 里没有核心那一份（或者没有这个文件）".to_string());
    }
    let readme = fs::read_to_string(dir.join("README.txt")).unwrap_or_default();
    if !readme.starts_with(&format!("Miyu {version} ({target})\n")) {
        problems.push(format!("README.txt 开头该写 Miyu {version} ({target})"));
    }

    // 比、报都用 `resources/` 下的相对路径、一律 `/`（[`relative`]）：`git ls-files` 写的是 `/`，包里走出来的照系统的分隔符，
    // Windows 上是 `\`。
    let shipped = dir.join("resources");
    let listed: std::collections::BTreeSet<String> =
        resources.iter().map(|file| relative(file)).collect();
    for inside in &listed {
        let from_repo = root.join("resources").join(inside);
        match fs::read(shipped.join(inside)) {
            Err(_) => problems.push(format!("少了资源 resources/{inside}")),
            Ok(bytes) if fs::read(&from_repo).ok().as_deref() != Some(bytes.as_slice()) => {
                problems.push(format!("资源 resources/{inside} 和仓库的不一样"));
            }
            Ok(_) => {}
        }
    }
    let mut entries = Vec::new();
    if shipped.is_dir() {
        walk(&shipped, Path::new(""), &mut entries)?;
    }
    let heads = ["packages/miyu-tui/", "packages/miyu-webui/"];
    for (inside, source) in &entries {
        let inside = relative(inside);
        let extra = source.is_some()
            && !listed.contains(&inside)
            && !heads.iter().any(|head| inside.starts_with(head));
        if extra {
            problems.push(format!("resources/{inside} 不在仓库的清单里"));
        }
    }
    let needed = [
        "packages/miyu-tui/package.toml",
        "packages/miyu-tui/LICENSE",
        "packages/miyu-tui/THIRD-PARTY-LICENSES.txt",
        "packages/miyu-webui/package.toml",
        "packages/miyu-webui/LICENSE",
        "packages/miyu-webui/THIRD-PARTY-LICENSES.txt",
        "packages/miyu-webui/pages/index.html",
    ];
    for file in needed {
        if !shipped.join(file).is_file() {
            problems.push(format!("少了 resources/{file}"));
        }
    }
    if shipped.join("packages/miyu-webui/bin").exists() {
        problems.push(
            "resources/packages/miyu-webui/bin/ 不该有：网页的程序放在 miyu 旁边".to_string(),
        );
    }
    Ok(problems)
}

/// `resources/` 下的相对路径，写成 `/` 分开的字：去掉开头的 `resources`，系统的分隔符和 Windows 写法的 `\\` 都换成 `/`。
pub(super) fn relative(path: &Path) -> String {
    let text = slashed(path).replace('\\', "/");
    match text.strip_prefix("resources/") {
        Some(inside) => inside.to_string(),
        None => text,
    }
}

/// 跑包里的 `miyu`：`--version` 报这一版；隔离的数据根 `home` 里 `miyu check` 要过，跑完叫核心停下。
fn runs(dir: &Path, home: &Path, version: &str) -> Vec<String> {
    let miyu = dir.join(format!("miyu{}", std::env::consts::EXE_SUFFIX));
    let mut problems = Vec::new();
    match isolated(&miyu, home).arg("--version").output() {
        Ok(done) if done.status.success() => {
            let said = String::from_utf8_lossy(&done.stdout).trim().to_string();
            if said != format!("miyu {version}") {
                problems.push(format!(
                    "miyu --version 报的是「{said}」，该是 miyu {version}"
                ));
            }
        }
        Ok(done) => problems.push(format!("miyu --version 没成功（{}）", done.status)),
        Err(error) => problems.push(format!("miyu 起不来（{error}）")),
    }
    match isolated(&miyu, home).arg("check").output() {
        Ok(done) if done.status.success() => {}
        Ok(done) => problems.push(format!(
            "miyu check 没过（{}）：{}{}",
            done.status,
            String::from_utf8_lossy(&done.stdout).trim(),
            String::from_utf8_lossy(&done.stderr).trim()
        )),
        Err(error) => problems.push(format!("miyu check 起不来（{error}）")),
    }
    if let Err(error) = isolated(&miyu, home).args(["core", "stop"]).status() {
        problems.push(format!("miyu core stop 起不来（{error}）"));
    }
    problems
}

/// 包里的 `miyu`，数据根指到 `home`，别的 `MIYU_` 开头的环境变量都去掉（尤其 `MIYU_RESOURCES`：要照旁边的 `resources/` 找）。
fn isolated(miyu: &Path, home: &Path) -> Command {
    let mut command = Command::new(miyu);
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("MIYU_") {
            command.env_remove(&name);
        }
    }
    command.env("MIYU_HOME", home);
    command
}

/// 能跑：Unix 上有执行位；Windows 上在就算。
fn runnable(file: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(file).is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        file.is_file()
    }
}

mod parts;

#[cfg(test)]
mod tests;
