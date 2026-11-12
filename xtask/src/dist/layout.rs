//! 搭目录：合许可证清单、写 README、把程序和资源摆好，和几样拷文件的小工具。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::archive::walk;
use crate::heads::Pin;

/// 合三份第三方许可证清单：核心的，终端、网页各自 `cargo xtask notices` 生成的，前面写清每一段是谁的、取的哪一份。
pub(super) fn notices(core: &str, tui: (&Pin, &str), web: (&Pin, &str)) -> String {
    let mut text = String::from(
        "Third-party licenses of Miyu / Miyu 的第三方许可证\n\n\
         This file joins three lists / 这份文件合了三份清单:\n\
         1. Miyu core: miyu, miyu-sandbox, miyu-extract / 核心\n",
    );
    text.push_str(&format!(
        "2. Terminal interface {}: {} @ {} / 终端界面\n",
        tui.0.id, tui.0.repo, tui.0.rev
    ));
    text.push_str(&format!(
        "3. Web interface {}: {} @ {} / 网页界面\n",
        web.0.id, web.0.repo, web.0.rev
    ));
    let parts = [
        ("1. Miyu core / 核心".to_string(), core),
        (format!("2. {} / 终端界面", tui.0.id), tui.1),
        (format!("3. {} / 网页界面", web.0.id), web.1),
    ];
    for (title, body) in parts {
        text.push_str(&format!(
            "\n{}\n{title}\n{}\n\n",
            "=".repeat(72),
            "=".repeat(72)
        ));
        text.push_str(body.trim_end());
        text.push('\n');
    }
    text
}

/// 压缩包里的 `README.txt`：中英两段。
pub(super) fn readme(version: &str, target: &str) -> String {
    let windows = target.starts_with("windows");
    let program = if windows { "miyu.exe" } else { "miyu" };
    let path_zh = if windows {
        "把这个目录加进 PATH（或者直接写 miyu.exe 的完整路径）"
    } else {
        "把这个目录加进 PATH，或者在 ~/.local/bin 里放一个指向 miyu 的链接"
    };
    let path_en = if windows {
        "Add this folder to PATH (or call miyu.exe by its full path)"
    } else {
        "Add this folder to PATH, or link miyu into ~/.local/bin"
    };
    format!(
        "Miyu {version} ({target})\n\
         \n\
         中文\n\
         \n\
         解开就能用，不要管理员。{path_zh}，然后：\n\
         \n\
         \x20 miyu        打开终端界面，第一次会问用哪个模型、选哪个人格\n\
         \x20 miyu web    打开网页界面\n\
         \x20 miyu --help 看全部命令\n\
         \n\
         程序照旁边的 resources/ 找资源，{program} 和 resources/ 要放在一起。终端界面、网页界面的程序也在这个目录里。\n\
         数据放在家目录的 .miyu 里（设了 MIYU_HOME 的放在它指的地方）。要开机就起的跑 miyu service enable。\n\
         卸载：先 miyu service disable（开过的话），再删掉这个目录。数据在 .miyu 里，不要了自己删。\n\
         \n\
         许可证：Miyu 用 AGPL-3.0-or-later，见 LICENSE；编进程序的第三方包见 THIRD-PARTY-LICENSES.txt。\n\
         \n\
         English\n\
         \n\
         Unpack and run; no administrator rights needed. {path_en}, then:\n\
         \n\
         \x20 miyu        open the terminal interface; the first run asks for a model and a persona\n\
         \x20 miyu web    open the web interface\n\
         \x20 miyu --help list every command\n\
         \n\
         The program finds its resources in resources/ next to it, so keep {program} and resources/ together. The terminal and\n\
         web interfaces live in this folder too. Data goes to .miyu in your home folder (or wherever MIYU_HOME points). To\n\
         start Miyu at login, run miyu service enable.\n\
         To uninstall: run miyu service disable (if you enabled it), then delete this folder. Your data stays in .miyu; delete\n\
         it yourself if you no longer want it.\n\
         \n\
         License: Miyu is AGPL-3.0-or-later, see LICENSE; third-party code built into the programs is listed in\n\
         THIRD-PARTY-LICENSES.txt.\n"
    )
}

/// 搭目录要的东西，都已经在磁盘上。
pub(super) struct Sources {
    /// 放在一起的程序（完整路径）。
    pub(super) programs: Vec<PathBuf>,
    /// 核心仓库的 `resources/`。
    pub(super) resources: PathBuf,
    /// 里面要带的文件，相对 `resources`。
    pub(super) resource_files: Vec<PathBuf>,
    /// 终端的包文件夹：源文件、在 `packages/tui/` 里的位置。
    pub(super) tui_package: Vec<(PathBuf, PathBuf)>,
    /// 网页 `pack-web.py --stage` 搭好的包文件夹，`bin/` 不带。
    pub(super) web_package: PathBuf,
    /// 核心的 `LICENSE`。
    pub(super) license: PathBuf,
    /// 合好的第三方许可证清单。
    pub(super) notices: String,
    /// `README.txt`。
    pub(super) readme: String,
}

/// 照 `sources` 搭 `dir`（不能已经有）。
pub(super) fn stage(sources: &Sources, dir: &Path) -> Result<(), String> {
    if dir.exists() {
        return Err(format!("{} 已经有了", dir.display()));
    }
    make_dir(dir)?;
    for program in &sources.programs {
        let name = program
            .file_name()
            .ok_or_else(|| format!("{} 不是文件", program.display()))?;
        let to = dir.join(name);
        copy(program, &to)?;
        executable(&to)?;
    }
    let resources = dir.join("resources");
    for file in &sources.resource_files {
        copy(&sources.resources.join(file), &resources.join(file))?;
    }
    let packages = resources.join("packages");
    for id in ["tui", "webui"] {
        if packages.join(id).exists() {
            return Err(format!(
                "核心的 resources/packages/ 里已经有 {id}，和头的包文件夹撞了"
            ));
        }
    }
    for (from, inside) in &sources.tui_package {
        copy(from, &packages.join("tui").join(inside))?;
    }
    copy_tree(&sources.web_package, &packages.join("webui"), &["bin"])?;
    copy(&sources.license, &dir.join("LICENSE"))?;
    write(&dir.join("THIRD-PARTY-LICENSES.txt"), &sources.notices)?;
    write(&dir.join("README.txt"), &sources.readme)?;
    Ok(())
}

pub(super) fn write(file: &Path, text: &str) -> Result<(), String> {
    fs::write(file, text).map_err(|error| format!("{}：{error}", file.display()))
}

pub(super) fn make_dir(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|error| format!("{}：{error}", dir.display()))
}

pub(super) fn remove(dir: &Path) -> Result<(), String> {
    match fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("{}：{error}", dir.display())),
    }
}

/// 拷一个文件，要的上一级先建上。
pub(super) fn copy(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        make_dir(parent)?;
    }
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|error| format!("{} → {}：{error}", from.display(), to.display()))
}

/// 整个拷一个目录，顶层名字在 `skip` 里的不拷。
pub(super) fn copy_tree(from: &Path, to: &Path, skip: &[&str]) -> Result<(), String> {
    let mut entries = Vec::new();
    walk(from, Path::new(""), &mut entries)?;
    make_dir(to)?;
    for (inside, source) in entries {
        let top = inside
            .components()
            .next()
            .map(|part| part.as_os_str().to_string_lossy().into_owned());
        if top.as_deref().is_some_and(|top| skip.contains(&top)) {
            continue;
        }
        match source {
            None => make_dir(&to.join(&inside))?,
            Some(source) => copy(&source, &to.join(&inside))?,
        }
    }
    Ok(())
}

/// 程序带上执行位（Unix）。
pub(super) fn executable(file: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(file, fs::Permissions::from_mode(0o755))
            .map_err(|error| format!("{}：{error}", file.display()))?;
    }
    #[cfg(not(unix))]
    let _ = file;
    Ok(())
}
