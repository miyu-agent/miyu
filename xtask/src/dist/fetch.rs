//! 取头、编、跑外面的命令：git、cargo、python。

use std::path::{Path, PathBuf};
use std::process::Command;

use super::layout::make_dir;
use crate::heads::Pin;

/// 外面这个 cargo 给子进程的、会把头的构建带偏的环境变量（`CARGO_PKG_` 开头的另算）：头照它自己的 `rust-toolchain.toml`、
/// 自己的 `target/` 编。
pub(super) const FOREIGN: [&str; 12] = [
    "RUSTUP_TOOLCHAIN",
    "CARGO",
    "CARGO_TARGET_DIR",
    "CARGO_MANIFEST_DIR",
    "CARGO_MANIFEST_PATH",
    "CARGO_PRIMARY_PACKAGE",
    "CARGO_CRATE_NAME",
    "CARGO_BIN_NAME",
    "CARGO_MAKEFLAGS",
    "RUSTC",
    "RUSTDOC",
    "RUSTC_WRAPPER",
];

/// 把头取进 `dir`，检出 `pin.rev`，清掉多余的文件（`target/` 留着），核对检出的提交就是它。
pub(super) fn fetch(dir: &Path, pin: &Pin) -> Result<(), String> {
    if dir.join(".git").is_dir() {
        status(git(dir).args(["fetch", "--quiet", "--tags", "--force", "origin"]))?;
    } else {
        if let Some(parent) = dir.parent() {
            make_dir(parent)?;
        }
        status(
            Command::new("git")
                .arg("clone")
                .arg("--quiet")
                .arg(&pin.repo)
                .arg(dir),
        )?;
    }
    status(git(dir).args([
        "-c",
        "advice.detachedHead=false",
        "checkout",
        "--quiet",
        "--force",
        "--detach",
        &pin.rev,
    ]))?;
    status(git(dir).args(["clean", "-ffdxq", "-e", "/target/"]))?;
    let head = output(git(dir).args(["rev-parse", "HEAD"]))?;
    let wanted = output(git(dir).args(["rev-parse", &format!("{}^{{commit}}", pin.rev)]))?;
    if head.trim() != wanted.trim() {
        return Err(format!(
            "{} 检出的是 {}，钉的是 {}",
            pin.id,
            head.trim(),
            wanted.trim()
        ));
    }
    Ok(())
}

/// `repo` 里 `under` 下登记了的文件，相对 `repo`。
pub(super) fn tracked(repo: &Path, under: &str) -> Result<Vec<PathBuf>, String> {
    let listed = output(git(repo).args(["ls-files", "-z", "--", under]))?;
    Ok(listed
        .split('\0')
        .filter(|name| !name.is_empty())
        .map(PathBuf::from)
        .collect())
}

/// cargo 的 target 目录（照 `cargo metadata`，设了 `CARGO_TARGET_DIR` 的照它）。
pub(crate) fn target_dir(root: &Path, cargo: &str) -> Result<PathBuf, String> {
    let text = output(Command::new(cargo).current_dir(root).args([
        "metadata",
        "--format-version",
        "1",
        "--no-deps",
    ]))?;
    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| format!("cargo metadata 的输出读不了（{error}）"))?;
    json["target_directory"]
        .as_str()
        .map(PathBuf::from)
        .ok_or_else(|| "cargo metadata 没给 target_directory".to_string())
}

/// 在头的检出里跑的 cargo：照 PATH 找（rustup 的代理认那个仓库的 `rust-toolchain.toml`），外面这个 cargo 的环境变量不带。
pub(super) fn head_cargo(dir: &Path) -> Command {
    let mut command = Command::new("cargo");
    command.current_dir(dir);
    for (name, _) in std::env::vars_os() {
        let text = name.to_string_lossy();
        if FOREIGN.contains(&text.as_ref()) || text.starts_with("CARGO_PKG_") {
            command.env_remove(&name);
        }
    }
    command
}

/// `python3`，没有的照 `python` 找（Windows 的机器上 python3 不一定在，同网页的发版模板）。
pub(super) fn python() -> Result<Command, String> {
    for name in ["python3", "python"] {
        let found = Command::new(name)
            .arg("--version")
            .output()
            .is_ok_and(|done| done.status.success());
        if found {
            return Ok(Command::new(name));
        }
    }
    Err("找不到 python3（网页的包文件夹照它的 tools/pack-web.py 搭）".to_string())
}

pub(super) fn git(dir: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir);
    command
}

/// 跑完要成功；输出照旧给终端。
pub(super) fn status(command: &mut Command) -> Result<(), String> {
    let done = command
        .status()
        .map_err(|error| format!("{command:?} 起不来（{error}）"))?;
    match done.success() {
        true => Ok(()),
        false => Err(format!("{command:?} 没成功（{done}）")),
    }
}

/// 跑完要成功，交回标准输出。
pub(super) fn output(command: &mut Command) -> Result<String, String> {
    let done = command
        .output()
        .map_err(|error| format!("{command:?} 起不来（{error}）"))?;
    if !done.status.success() {
        return Err(format!(
            "{command:?} 没成功（{}）：{}",
            done.status,
            String::from_utf8_lossy(&done.stderr).trim()
        ));
    }
    String::from_utf8(done.stdout).map_err(|_| format!("{command:?} 的输出不是 UTF-8"))
}
