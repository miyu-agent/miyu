//! 问哪几家、先问谁（照这台电脑的平台、桌面）：每一家交图的路径，没装、没设的交 `None`；读回答在 `parse.rs`。

use std::io;
use std::path::PathBuf;

#[cfg(not(all(unix, not(target_os = "macos"))))]
use crate::parse;

#[cfg(all(unix, not(target_os = "macos")))]
pub use crate::unix::sources;

/// 问一家：照深浅交图的路径。
pub type Ask = fn(bool) -> io::Result<Option<PathBuf>>;

/// 这台电脑上依次问的几家。
#[cfg(target_os = "macos")]
pub fn sources() -> Vec<(&'static str, Ask)> {
    vec![("macos", macos)]
}

/// 这台电脑上依次问的几家。
#[cfg(windows)]
pub fn sources() -> Vec<(&'static str, Ask)> {
    vec![("windows", windows)]
}

/// macOS：先读系统存壁纸设置的文件（`plutil` 转成 XML 找 `file://`，不用申请权限）；读不到的（老系统、系统自带的动态壁纸）再问
/// System Events（第一次会问要不要允许，不允许的出错退出，就是读不到）。
#[cfg(target_os = "macos")]
fn macos(_dark: bool) -> io::Result<Option<PathBuf>> {
    if let Some(home) = home() {
        let index = home.join("Library/Application Support/com.apple.wallpaper/Store/Index.plist");
        if index.is_file() {
            let index = index.to_string_lossy().into_owned();
            if let Some(path) = crate::run("plutil", &["-convert", "xml1", "-o", "-", &index])?
                .as_deref()
                .and_then(parse::macos_index)
            {
                return Ok(Some(path));
            }
        }
    }
    let script = "tell application \"System Events\" to get picture of current desktop";
    Ok(crate::run("osascript", &["-e", script])?
        .as_deref()
        .and_then(parse::osascript_path))
}

/// Windows：注册表里的 `WallPaper`；空的、读不了的（幻灯片放映）读 Windows 转好的那一张 `TranscodedWallpaper`（没有扩展名的
/// JPEG，照文件开头认）。
#[cfg(windows)]
fn windows(_dark: bool) -> io::Result<Option<PathBuf>> {
    let set = crate::run(
        "reg",
        &["query", r"HKCU\Control Panel\Desktop", "/v", "WallPaper"],
    )?
    .as_deref()
    .and_then(parse::reg_wallpaper)
    .filter(|p| p.is_file());
    if set.is_some() {
        return Ok(set);
    }
    let transcoded = std::env::var_os("APPDATA")
        .map(|d| PathBuf::from(d).join(r"Microsoft\Windows\Themes\TranscodedWallpaper"));
    Ok(transcoded.filter(|p| p.is_file()))
}

/// 家目录。
#[cfg_attr(windows, allow(dead_code))]
pub(crate) fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// 读一个文字文件：没有的是 `None`。
#[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
pub(crate) fn read_if_exists(path: &std::path::Path) -> io::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
