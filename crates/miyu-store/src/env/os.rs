//! 本机是什么系统（施工 2-7 再补，`26-提示词.md` 第四节第 3 块）：写进 system 核心的几行，她照它挑包管理器、写命令，不用先跑
//! 一遍 `cat /etc/os-release`。核心头一次要时认一次，之后照它；认不出的照平台写 `Linux`、`macOS`、`Windows`。
//!
//! 只写系统叫什么、大版本：小版本每个月都变，变了新开的会话 system 就和昨天的不一样，跨会话的缓存接不上。

use std::sync::OnceLock;

use super::Platform;

#[cfg(test)]
mod tests;

/// 系统的名字最长几个字：`/etc/os-release` 是发行版写的，太长的截掉，进 system 的只有一小截。
const MAX_CHARS: usize = 64;

/// 本机系统的名字：Linux 照 `/etc/os-release` 的 `NAME` 加 `VERSION_ID`（`Arch Linux`、`Ubuntu 24.04`），macOS 照系统
/// 版本的大版本号（`macOS 15`），Windows 照内部版本号分 10 和 11（`Windows 11`），服务器版写 `Windows Server`。
pub fn os_name() -> &'static str {
    static NAME: OnceLock<String> = OnceLock::new();
    NAME.get_or_init(|| detect(Platform::current()))
}

/// 照平台认一次。
fn detect(platform: Platform) -> String {
    let named = match platform {
        Platform::Linux => ["/etc/os-release", "/usr/lib/os-release"]
            .into_iter()
            .find_map(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| linux(&text)),
        Platform::Macos => {
            std::fs::read_to_string("/System/Library/CoreServices/SystemVersion.plist")
                .ok()
                .and_then(|text| macos(&text))
        }
        Platform::Windows => windows_version().map(|(build, server)| windows(build, server)),
    };
    named.unwrap_or_else(|| fallback(platform).to_string())
}

/// 认不出的照平台写。
fn fallback(platform: Platform) -> &'static str {
    match platform {
        Platform::Linux => "Linux",
        Platform::Macos => "macOS",
        Platform::Windows => "Windows",
    }
}

/// `os-release` 的 `NAME` 加 `VERSION_ID`（滚动更新的发行版没有版本号，只写名字）；没有 `NAME` 的认不出。
fn linux(os_release: &str) -> Option<String> {
    let field = |key: &str| {
        os_release.lines().find_map(|line| {
            let value = line.trim().strip_prefix(key)?.strip_prefix('=')?;
            let value = unquote(value.trim());
            (!value.is_empty()).then_some(value)
        })
    };
    let name = field("NAME")?;
    Some(clean(&match field("VERSION_ID") {
        Some(version) => format!("{name} {version}"),
        None => name,
    }))
}

/// 去掉 `os-release` 值两头的引号，引号里的 `\"` 这类转义照字面还原。
fn unquote(value: &str) -> String {
    let quoted = ['"', '\''].into_iter().find_map(|quote| {
        value
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
    });
    match quoted {
        Some(inner) => inner.replace("\\\"", "\"").replace("\\\\", "\\"),
        None => value.to_string(),
    }
}

/// `SystemVersion.plist` 里 `ProductVersion` 的大版本号：`15.3.1` 写 `macOS 15`。
fn macos(plist: &str) -> Option<String> {
    let (_, after) = plist.split_once("<key>ProductVersion</key>")?;
    let (_, value) = after.split_once("<string>")?;
    let (version, _) = value.split_once("</string>")?;
    let major = version.trim().split('.').next()?;
    (!major.is_empty() && major.chars().all(|c| c.is_ascii_digit()))
        .then(|| format!("macOS {major}"))
}

/// Windows 11 的内部版本号从 22000 起，之前的是 Windows 10。
fn windows(build: u32, server: bool) -> String {
    match (server, build >= 22000) {
        (true, _) => "Windows Server".to_string(),
        (false, true) => "Windows 11".to_string(),
        (false, false) => "Windows 10".to_string(),
    }
}

/// 本机 Windows 的内部版本号，是不是服务器版。
#[cfg(windows)]
fn windows_version() -> Option<(u32, bool)> {
    Some((
        windows_version::OsVersion::current().build,
        windows_version::is_server(),
    ))
}

/// 不是 Windows 的没有。
#[cfg(not(windows))]
fn windows_version() -> Option<(u32, bool)> {
    None
}

/// 进 system 的只留一行：控制字符换成空格，连着的空白并成一个，太长的截掉。
fn clean(name: &str) -> String {
    let spaced: String = name
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    spaced
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_CHARS)
        .collect()
}
