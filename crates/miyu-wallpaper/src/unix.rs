//! Linux、BSD 上问哪几家、先问谁（`probe.rs` 交出去）：认得出的桌面先问它自己的设置（KDE、GNOME、XFCE、Cinnamon、MATE），
//! 再问窗口管理器上常用的壁纸程序（awww、swww、hyprpaper、swaybg、wpaperd、waypaper）；X11 上最后问 feh、nitrogen 记下的那一张
//! （Wayland 上不问：那两个文件多半是以前留下的）。读回答在 `parse.rs`。

use std::io;
use std::path::PathBuf;

use crate::parse;
use crate::probe::{Ask, home, read_if_exists};

/// 这台电脑上依次问的几家。
pub fn sources() -> Vec<(&'static str, Ask)> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut out: Vec<(&'static str, Ask)> = Vec::new();
    // 认得出桌面的先问它自己的设置
    if desktop.contains("kde") {
        out.push(("kde", kde));
    }
    if ["gnome", "unity", "budgie", "pantheon"]
        .iter()
        .any(|d| desktop.contains(d))
    {
        out.push(("gnome", gnome));
    }
    if desktop.contains("xfce") {
        out.push(("xfce", xfce));
    }
    if desktop.contains("cinnamon") {
        out.push(("cinnamon", cinnamon));
    }
    if desktop.contains("mate") {
        out.push(("mate", mate));
    }
    // 窗口管理器（niri、sway、Hyprland……）上常用的几个壁纸程序
    out.push(("awww", |_| swww("awww")));
    out.push(("swww", |_| swww("swww")));
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        out.push(("hyprpaper", hyprpaper));
    }
    #[cfg(target_os = "linux")]
    out.push(("swaybg", swaybg));
    out.push(("wpaperd", wpaperd));
    out.push(("waypaper", waypaper));
    // X11 上的：Wayland 上这两个文件多半是以前留下的
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        out.push(("feh", feh));
        out.push(("nitrogen", nitrogen));
    }
    out
}

/// 配置目录：`$XDG_CONFIG_HOME`，没设的 `~/.config`。
fn config_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| home().map(|h| h.join(".config")))
}

/// XFCE：`xfce4-desktop.xml` 里的 `last-image`。
fn xfce(_dark: bool) -> io::Result<Option<PathBuf>> {
    let Some(dir) = config_dir() else {
        return Ok(None);
    };
    Ok(
        read_if_exists(&dir.join("xfce4/xfconf/xfce-perchannel-xml/xfce4-desktop.xml"))?
            .as_deref()
            .and_then(parse::xfce_last_image),
    )
}

/// Cinnamon：同 GNOME 的写法，换一个设置。
fn cinnamon(_dark: bool) -> io::Result<Option<PathBuf>> {
    Ok(crate::run(
        "gsettings",
        &["get", "org.cinnamon.desktop.background", "picture-uri"],
    )?
    .as_deref()
    .and_then(parse::gsettings_uri))
}

/// MATE：`picture-filename` 是路径（带引号，不是网址）。
fn mate(_dark: bool) -> io::Result<Option<PathBuf>> {
    Ok(crate::run(
        "gsettings",
        &["get", "org.mate.background", "picture-filename"],
    )?
    .as_deref()
    .and_then(parse::gsettings_uri))
}

/// swaybg：看正在跑的 swaybg 进程的参数（`/proc/<编号>/cmdline`）。
#[cfg(target_os = "linux")]
fn swaybg(_dark: bool) -> io::Result<Option<PathBuf>> {
    for entry in std::fs::read_dir("/proc")? {
        let dir = entry?.path();
        // 进程表里的东西随时会没：读不了的跳过，不算错
        let Ok(comm) = std::fs::read_to_string(dir.join("comm")) else {
            continue;
        };
        if comm.trim() != "swaybg" {
            continue;
        }
        let Ok(raw) = std::fs::read(dir.join("cmdline")) else {
            continue;
        };
        let args: Vec<String> = raw
            .split(|b| *b == 0)
            .map(|a| String::from_utf8_lossy(a).into_owned())
            .collect();
        if let Some(path) = parse::swaybg_image(&args) {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// wpaperd：`wpaperctl all-wallpapers`。
fn wpaperd(_dark: bool) -> io::Result<Option<PathBuf>> {
    Ok(crate::run("wpaperctl", &["all-wallpapers"])?
        .as_deref()
        .and_then(parse::wpaperd_all))
}

/// waypaper：它的 `config.ini`（它记着上一次设的那一张）。
fn waypaper(_dark: bool) -> io::Result<Option<PathBuf>> {
    let (Some(dir), Some(home)) = (config_dir(), home()) else {
        return Ok(None);
    };
    let home = home.to_string_lossy().into_owned();
    Ok(read_if_exists(&dir.join("waypaper/config.ini"))?
        .and_then(|t| parse::waypaper_ini(&t, &home)))
}

/// feh：`~/.fehbg`。
fn feh(_dark: bool) -> io::Result<Option<PathBuf>> {
    let Some(home) = home() else { return Ok(None) };
    Ok(read_if_exists(&home.join(".fehbg"))?
        .as_deref()
        .and_then(parse::fehbg))
}

/// nitrogen：`bg-saved.cfg`。
fn nitrogen(_dark: bool) -> io::Result<Option<PathBuf>> {
    let Some(dir) = config_dir() else {
        return Ok(None);
    };
    Ok(read_if_exists(&dir.join("nitrogen/bg-saved.cfg"))?
        .as_deref()
        .and_then(parse::nitrogen_cfg))
}

/// awww、swww：`query`。
fn swww(program: &str) -> io::Result<Option<PathBuf>> {
    Ok(crate::run(program, &["query"])?
        .as_deref()
        .and_then(parse::swww_query))
}

/// Hyprland 的 hyprpaper。
fn hyprpaper(_dark: bool) -> io::Result<Option<PathBuf>> {
    Ok(crate::run("hyprctl", &["hyprpaper", "listactive"])?
        .as_deref()
        .and_then(parse::hyprpaper_active))
}

/// GNOME：深色先问 `picture-uri-dark`，没设的照 `picture-uri`。
fn gnome(dark: bool) -> io::Result<Option<PathBuf>> {
    let keys: &[&str] = if dark {
        &["picture-uri-dark", "picture-uri"]
    } else {
        &["picture-uri"]
    };
    for key in keys {
        if let Some(path) = crate::run("gsettings", &["get", "org.gnome.desktop.background", key])?
            .as_deref()
            .and_then(parse::gsettings_uri)
        {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// KDE Plasma：读配置文件（`$XDG_CONFIG_HOME`，没设的 `~/.config`）；设的是壁纸包的，照深浅挑包里的一张。
fn kde(dark: bool) -> io::Result<Option<PathBuf>> {
    let Some(config) = config_dir() else {
        return Ok(None);
    };
    let Some(text) = read_if_exists(&config.join("plasma-org.kde.plasma.desktop-appletsrc"))?
    else {
        return Ok(None);
    };
    let Some(path) = parse::kde_image(&text) else {
        return Ok(None);
    };
    if !path.is_dir() {
        return Ok(Some(path));
    }
    let dirs: &[&str] = if dark {
        &["contents/images_dark", "contents/images"]
    } else {
        &["contents/images"]
    };
    for dir in dirs {
        let dir = path.join(dir);
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };
        let mut names = Vec::new();
        for entry in entries {
            // 名字不是 UTF-8 的不会是照分辨率起的名，不挑
            if let Ok(name) = entry?.file_name().into_string() {
                names.push(name);
            }
        }
        if let Some(name) = parse::kde_package_pick(&names) {
            return Ok(Some(dir.join(name)));
        }
    }
    Ok(None)
}
