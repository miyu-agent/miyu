//! `cargo xtask version <年.月.序号>`（施工 VR-1，设计 `59-发版.md` 第十节）：改工作区的版本（`Cargo.toml` 的
//! `[workspace.package] version`），跑一次 `cargo metadata` 让 `Cargo.lock` 里工作区自己那几个 crate 跟着改，再把两边读回来查一遍。
//!
//! 只收正式版的写法：年四位、月 1 到 12、这个月第几版从 0 数起，月和序号都不补零（`2026.11.0`、`2026.12.3`）。预发布
//! （`-rc.N`）不经这里：演练标签由 `release.yml` 在 CI 机器上临时改，不提交；演练前先用这个命令定好那一版的号。
//! `release.yml` 打包前照同一条规矩查（写成 bash 的正则，测试照 `PATTERN` 守着两边一样）。

use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

/// 正式版的写法，写成 `release.yml` 里 bash 认的正则：年四位、不以 0 开头；月 1 到 12；序号 0 或者不以 0 开头的数。[`check`] 照同样的
/// 规矩一段段查（说得清哪一段不对），这一条只给测试拿去和 `release.yml` 对。
#[cfg(test)]
pub(crate) const PATTERN: &str = r"^[1-9][0-9]{3}\.([1-9]|1[0-2])\.(0|[1-9][0-9]*)$";

/// `cargo xtask version <版本>`：版本照命令行，cargo 照 `CARGO`。
pub fn run(root: &Path) -> ExitCode {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let args: Vec<String> = std::env::args().skip(2).collect();
    match set(root, &cargo, &args) {
        Ok(version) => {
            println!("工作区版本改成了 {version}，Cargo.lock 跟着改了");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("version：{error}");
            ExitCode::FAILURE
        }
    }
}

fn set(root: &Path, cargo: &str, args: &[String]) -> Result<String, String> {
    let usage = "用法：cargo xtask version <年.月.序号>，例如 2026.11.0";
    let [version] = args else {
        return Err(usage.to_string());
    };
    check(version)?;
    let manifest = root.join("Cargo.toml");
    let text = read(&manifest)?;
    fs::write(&manifest, rewrite(&text, version)?)
        .map_err(|error| format!("{}：{error}", manifest.display()))?;
    let status = Command::new(cargo)
        .args(["metadata", "--format-version", "1"])
        .current_dir(root)
        .stdout(std::process::Stdio::null())
        .status()
        .map_err(|error| format!("跑不了 cargo metadata：{error}"))?;
    if !status.success() {
        return Err(format!(
            "cargo metadata 没成（{status}），Cargo.lock 可能还是旧的版本"
        ));
    }
    // 读回来再查一遍：两边都得是这一版、写法都对。
    let now = workspace_version(&read(&manifest)?).ok_or("改完读不出工作区的版本")?;
    check(&now)?;
    if now != *version {
        return Err(format!("工作区版本没改成 {version}，现在是 {now}"));
    }
    let locked = locked_version(&read(&root.join("Cargo.lock"))?, "miyu")
        .ok_or("Cargo.lock 里找不到 miyu")?;
    if locked != *version {
        return Err(format!(
            "Cargo.lock 里 miyu 还是 {locked}，没跟着改成 {version}"
        ));
    }
    Ok(now)
}

/// 版本写法对不对：年.月.序号，不对的说清哪里不对。
pub(crate) fn check(version: &str) -> Result<(), String> {
    let wrong = |why: &str| {
        Err(format!(
            "版本 {version} 不对：{why}（要 年.月.序号，例如 2026.11.0；月、序号不补零）"
        ))
    };
    let parts: Vec<&str> = version.split('.').collect();
    let [year, month, nth] = parts.as_slice() else {
        return wrong("要正好三段");
    };
    if !parts
        .iter()
        .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return wrong("每一段都要是数字");
    }
    if year.len() != 4 || year.starts_with('0') {
        return wrong("年要四位");
    }
    if month.starts_with('0') || !(1..=12).contains(&month.parse::<u32>().unwrap_or(0)) {
        return wrong("月要 1 到 12、不补零");
    }
    if nth.len() > 1 && nth.starts_with('0') {
        return wrong("序号不补零");
    }
    Ok(())
}

/// 把 `[workspace.package]` 里那一行 `version = "…"` 换成 `version`：正好一行，不是的报错。
pub(crate) fn rewrite(manifest: &str, version: &str) -> Result<String, String> {
    let mut inside = false;
    let mut replaced = 0;
    let mut out = String::with_capacity(manifest.len());
    for line in manifest.split_inclusive('\n') {
        let bare = line.trim_end();
        if bare.starts_with('[') {
            inside = bare == "[workspace.package]";
        }
        if inside && bare.starts_with("version = \"") && bare.ends_with('"') {
            replaced += 1;
            out.push_str(&format!("version = \"{version}\""));
            out.push_str(&line[bare.len()..]);
        } else {
            out.push_str(line);
        }
    }
    match replaced {
        1 => Ok(out),
        0 => Err("Cargo.toml 的 [workspace.package] 里没有 version = \"…\" 那一行".to_string()),
        _ => Err("Cargo.toml 的 [workspace.package] 里有不止一行 version".to_string()),
    }
}

/// `[workspace.package]` 里写的版本。
pub(crate) fn workspace_version(manifest: &str) -> Option<String> {
    let mut inside = false;
    for line in manifest.lines() {
        if line.starts_with('[') {
            inside = line.trim_end() == "[workspace.package]";
        } else if inside && let Some(rest) = line.strip_prefix("version = \"") {
            return rest.trim_end().strip_suffix('"').map(str::to_string);
        }
    }
    None
}

/// `Cargo.lock` 里包 `name` 的版本：`name = "<名字>"` 下一行的 `version`。
pub(crate) fn locked_version(lock: &str, name: &str) -> Option<String> {
    let wanted = format!("name = \"{name}\"");
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if line == wanted {
            let next = lines.next()?;
            return next
                .strip_prefix("version = \"")?
                .strip_suffix('"')
                .map(str::to_string);
        }
    }
    None
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("{}：{error}", path.display()))
}

#[cfg(test)]
mod tests;
