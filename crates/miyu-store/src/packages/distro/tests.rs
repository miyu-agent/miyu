//! 系统加装那几处：指定的只看它；Linux 照 XDG（没设照默认，先列的优先，相对的不算、重复的留一次）；macOS 照 Homebrew 的前缀、
//! 不解开链接；Windows 没有；和出厂的同一处的去掉。

use super::*;

fn place(platform: Platform) -> Where {
    Where {
        platform,
        only: None,
        xdg_data_dirs: None,
        called: None,
    }
}

// 路径写的是 Unix 的绝对路径：Windows 上它们不算绝对路径。
#[cfg(unix)]
#[test]
fn linux_follows_xdg_data_dirs_in_order() {
    let shipped = Path::new("/opt/miyu/resources/packages");
    let mut linux = place(Platform::Linux);
    assert_eq!(
        dirs(&linux, shipped),
        vec![
            PathBuf::from("/usr/local/share/miyu/packages"),
            PathBuf::from("/usr/share/miyu/packages")
        ],
        "没设照默认"
    );
    linux.xdg_data_dirs = Some(OsString::from(""));
    assert_eq!(dirs(&linux, shipped).len(), 2, "空的也照默认");
    linux.xdg_data_dirs = Some(OsString::from(
        "/home/a/.local/share:relative:/usr/share::/home/a/.local/share",
    ));
    assert_eq!(
        dirs(&linux, shipped),
        vec![
            PathBuf::from("/home/a/.local/share/miyu/packages"),
            PathBuf::from("/usr/share/miyu/packages")
        ],
        "相对的不算，重复的留第一次"
    );
}

// 路径写的是 Unix 的绝对路径：Windows 上它们不算绝对路径。
#[cfg(unix)]
#[test]
fn macos_uses_the_homebrew_prefix_of_the_called_path() {
    let mut mac = place(Platform::Macos);
    mac.called = Some(PathBuf::from("/opt/homebrew/bin/miyu"));
    assert_eq!(
        dirs(
            &mac,
            Path::new("/opt/homebrew/Cellar/miyu-core/1.0.0/libexec/resources/packages")
        ),
        vec![PathBuf::from("/opt/homebrew/share/miyu/packages")]
    );
    mac.called = None;
    assert!(dirs(&mac, Path::new("/x")).is_empty());
}

#[test]
fn windows_has_none_and_the_variable_wins_everywhere() {
    let mut windows = place(Platform::Windows);
    assert!(dirs(&windows, Path::new("/x")).is_empty());
    let only = std::env::temp_dir().join("miyu-distro-only");
    windows.only = Some(only.clone().into_os_string());
    assert_eq!(dirs(&windows, Path::new("/x")), vec![only.clone()]);
    let mut linux = place(Platform::Linux);
    linux.only = Some(only.clone().into_os_string());
    linux.xdg_data_dirs = Some(OsString::from("/usr/share"));
    assert_eq!(dirs(&linux, Path::new("/x")), vec![only]);
    linux.only = Some(OsString::from("relative/dir"));
    assert!(dirs(&linux, Path::new("/x")).is_empty(), "相对的不认");
    linux.only = Some(OsString::new());
    linux.platform = Platform::Windows;
    assert!(dirs(&linux, Path::new("/x")).is_empty(), "空的当没设");
}

// 路径写的是 Unix 的绝对路径：Windows 上它们不算绝对路径。
#[cfg(unix)]
#[test]
fn the_shipped_folder_is_not_also_a_distro_one() {
    let mut linux = place(Platform::Linux);
    linux.xdg_data_dirs = Some(OsString::from("/usr/share:/usr/local/share"));
    assert_eq!(
        dirs(&linux, Path::new("/usr/share/miyu/packages")),
        vec![PathBuf::from("/usr/local/share/miyu/packages")]
    );
}
