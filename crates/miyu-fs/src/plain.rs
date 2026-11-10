//! 给人、给头看的路径（施工 WP-1）：Windows 上换真实位置（`canonicalize`）会加上扩展长度的前缀 `\\?\`，头拿它去列目录、人看到
//! 它都不认；核心自己用的照旧带着，交出去之前去掉。原来基础系统、端点各写了一份，挪到这里共用。

use std::path::{Path, PathBuf};

/// 绝对路径去掉 Windows 换真实位置时加上的 `\\?\`：`\\?\C:\x` 写成 `C:\x`，`\\?\UNC\host\share` 写成
/// `\\host\share`。别的平台上不会有这个前缀，照原样。
pub fn plain(real: &Path) -> String {
    let text = real.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    match text.strip_prefix(r"\\?\") {
        Some(rest) => rest.to_string(),
        None => text.into_owned(),
    }
}

/// Windows 的普通写法里另有意思的名字：设备名（不管大小写、带不带扩展名）。
const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// 普通写法最长几个字（`MAX_PATH` 260 减结尾的零）。
const MOST: usize = 259;

/// 交给别的程序用的路径（施工 FH-3下补，虚拟机上实测发现）：Windows 上换真实位置带的 `\\?\`，cmd 的 `cd` 和不少老工具认不了。
/// 普通写法表示得了的去掉前缀（同 [`plain`]）；表示不了的照原样：超过 259 个字、哪一段是设备名（`CON`、`nul.txt` 这些）、
/// 哪一段结尾是点或空格（普通写法会被系统去掉）。没有这个前缀的照原样。
pub fn usable(real: &Path) -> PathBuf {
    let text = real.to_string_lossy();
    if !text.starts_with(r"\\?\") {
        return real.to_path_buf();
    }
    let short = plain(real);
    let reserved = |name: &str| {
        let stem = name.split('.').next().unwrap_or(name);
        RESERVED.iter().any(|one| one.eq_ignore_ascii_case(stem))
    };
    let odd = short
        .split('\\')
        .skip(1)
        .any(|name| reserved(name) || name.ends_with('.') || name.ends_with(' '));
    if odd || short.chars().count() > MOST {
        return real.to_path_buf();
    }
    PathBuf::from(short)
}

/// 会话的工作目录 `cwd` 交给扩展用的写法（施工 XW-1）：会话里记的是人写的样子（`~`、`~/src/miyu`），扩展拿字面的 `~` 去换真实
/// 位置必然失败。`~` 照 `home` 接上、换成真实位置、写成 [`usable`] 的样子；换不成的（家目录不知道、读不了）照原样。
#[must_use]
pub fn handed(cwd: &str, home: Option<&Path>) -> String {
    match crate::resolve(Path::new(""), home, cwd) {
        Ok(real) => usable(&real).to_string_lossy().into_owned(),
        Err(_) => cwd.to_string(),
    }
}

#[cfg(test)]
mod tests;
