//! 头的版本（施工 RL-6，主人定过四个头同一个版本号，core1 11-07 照推荐定 A）：取到头以后、编之前，照核心的版本对头。
//!
//! - 核心是演练号（`<版本>-rc.N`，发版流程在 CI 机器上把工作区临时改成标签里的版本，RL-2补2）：头的几处版本也临时改成它，再在头
//!   的检出里跑一次 `cargo metadata` 把锁文件跟着刷新（后面带 `--locked` 的编译才不拒），不提交。三个分包、一体包里的程序都自报
//!   同一个 rc 号，`miyu upgrade` 照标签找得到、核得上。
//! - 核心是正式号：头的几处版本要和它一样，不一样的当场停下，先把 `heads.toml` 钉到定了号的提交。
//!
//! 几处照 TOML 的段找 `version = "…"` 那一行（[`PLACES`]），改的只有那一行，别的一个字节不动。

use std::path::Path;

use super::fetch::{head_cargo, status};

/// 一个头里写着版本的一处：文件（相对检出）、在哪一段。
pub(super) struct Place {
    pub(super) file: &'static str,
    pub(super) section: &'static str,
}

/// 每个头写着版本的几处：终端的 `[package]`、网页工作区的 `[workspace.package]`，加上各自包清单的 `[package]`。
pub(super) const PLACES: [(&str, &[Place]); 2] = [
    (
        "tui",
        &[
            Place {
                file: "Cargo.toml",
                section: "package",
            },
            Place {
                file: "tui/package.toml",
                section: "package",
            },
        ],
    ),
    (
        "webui",
        &[
            Place {
                file: "Cargo.toml",
                section: "workspace.package",
            },
            Place {
                file: "webui/package.toml",
                section: "package",
            },
        ],
    ),
];

/// 核心的版本是不是演练号。
pub(super) fn rehearsal(version: &str) -> bool {
    version.contains("-rc.")
}

/// 照核心的版本 `version` 对编号 `id`、检出在 `dir` 的头：演练号的盖上、刷新锁文件；正式号的查一遍。
///
/// # Errors
///
/// 哪一处读不了、写不进、没有 `version`；正式号时头的版本和核心的不一样；刷新锁文件失败。
pub(super) fn head(id: &str, dir: &Path, version: &str) -> Result<(), String> {
    let places = PLACES
        .iter()
        .find(|(head, _)| *head == id)
        .map(|(_, places)| *places)
        .ok_or_else(|| format!("不认识的头 {id}：dist/stamp.rs 的 PLACES 里没有它"))?;
    for place in places {
        let path = dir.join(place.file);
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("读不了 {}：{error}", path.display()))?;
        if rehearsal(version) {
            let stamped = stamped(&text, place.section, version).ok_or_else(|| {
                format!("{} 的 [{}] 里没有 version", path.display(), place.section)
            })?;
            std::fs::write(&path, stamped)
                .map_err(|error| format!("写不进 {}：{error}", path.display()))?;
            continue;
        }
        let found = version_in(&text, place.section)
            .ok_or_else(|| format!("{} 的 [{}] 里没有 version", path.display(), place.section))?;
        if found != version {
            return Err(format!(
                "头 {id} 的 {}（[{}]）版本是 {found}，核心是 {version}：四个头同一个版本号，先把 heads.toml 钉到定了号的提交",
                place.file, place.section
            ));
        }
    }
    if rehearsal(version) {
        status(
            head_cargo(dir)
                .args(["metadata", "--format-version", "1"])
                .stdout(std::process::Stdio::null()),
        )?;
    }
    Ok(())
}

/// `text` 里 `[section]` 那一段的 `version = "…"` 换成 `version`；那一段没有的没有。
pub(super) fn stamped(text: &str, section: &str, version: &str) -> Option<String> {
    let mut current = String::new();
    let mut done = false;
    let mut out = String::with_capacity(text.len() + version.len());
    for line in text.split_inclusive('\n') {
        if let Some(name) = header(line) {
            current = name.to_string();
        } else if !done && current == section && line.trim_start().starts_with("version = \"") {
            let ending = &line[line.trim_end_matches(['\r', '\n']).len()..];
            out.push_str(&format!("version = \"{version}\"{ending}"));
            done = true;
            continue;
        }
        out.push_str(line);
    }
    done.then_some(out)
}

/// `text` 里 `[section]` 那一段的 `version`。
pub(super) fn version_in(text: &str, section: &str) -> Option<String> {
    let mut current = "";
    for line in text.lines() {
        if let Some(name) = header(line) {
            current = name;
        } else if current == section
            && let Some(rest) = line.trim_start().strip_prefix("version = \"")
        {
            return rest.split_once('"').map(|(value, _)| value.to_string());
        }
    }
    None
}

/// 一行是段的标题的，段名：`[name]` 是 `name`，数组段 `[[name]]` 是 `[name]`（和哪一段都对不上）；别的行没有。
fn header(line: &str) -> Option<&str> {
    let line = line.trim();
    Some(line.strip_prefix('[')?.strip_suffix(']')?.trim())
}

#[cfg(test)]
mod tests;
