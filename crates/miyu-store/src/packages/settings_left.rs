//! 卸包留下的设置项是谁的（施工 PK-5补）：照 pacman，不带 `purge` 卸包时 `etc/config.toml` 里它的设置项原样留着。出厂的包卸了
//! 清单还在资源目录里，前缀照它认；你装的包卸了整个文件夹都没了，前缀记在这里，核心读配置时照它认出这几个键是它留下的、
//! 不报不认识。
//!
//! 一个包一份 `etc/leftover/<编号>.toml`（[`DataRoot::leftover_settings`]），一行 `prefix = "<前缀>"`。只在真留了设置项时写；
//! `purge` 的、重装同编号的删掉。读不懂的、没写前缀的当前缀就是编号（清单没写 `prefix` 的默认）。

use std::fs;
use std::io;

use crate::root::DataRoot;

/// 记下你装的包 `id` 卸掉了、留下了前缀是 `prefix` 的设置项。
///
/// # Errors
///
/// 建不了目录、写不进。
pub fn mark(root: &DataRoot, id: &str, prefix: &str) -> io::Result<()> {
    fs::create_dir_all(root.leftover_settings_dir())?;
    // 前缀的写法同包编号（`miyu_config::key::ID`），不用转义。
    fs::write(
        root.leftover_settings(id),
        format!("prefix = \"{prefix}\"\n"),
    )
}

/// 删掉包 `id` 的记号：`purge` 了、重装了。本来就没有的不算错。
///
/// # Errors
///
/// 删不掉。
pub fn unmark(root: &DataRoot, id: &str) -> io::Result<()> {
    match fs::remove_file(root.leftover_settings(id)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// 包 `id` 有没有记号。
pub fn has(root: &DataRoot, id: &str) -> bool {
    root.leftover_settings(id).is_file()
}

/// 记着的每一个：编号和前缀，照编号排。文件名合编号写法、后缀是 `.toml` 的普通文件才算；目录读不了的当没有。
pub fn read(root: &DataRoot) -> Vec<(String, String)> {
    let Ok(entries) = fs::read_dir(root.leftover_settings_dir()) else {
        return Vec::new();
    };
    let mut found: Vec<(String, String)> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let id = name.strip_suffix(".toml")?.to_string();
            if !crate::personas::valid(&id) {
                return None;
            }
            let prefix = fs::read_to_string(entry.path())
                .ok()
                .and_then(|text| miyu_config::plain::json_at(&text, "prefix"))
                .and_then(|value| value.as_str().map(str::to_string))
                .filter(|prefix| miyu_config::key::valid(miyu_config::key::ID, prefix))
                .unwrap_or_else(|| id.clone());
            Some((id, prefix))
        })
        .collect();
    found.sort();
    found
}

#[cfg(test)]
mod tests;
