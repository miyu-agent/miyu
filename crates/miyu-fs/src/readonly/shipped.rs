//! 出厂的只禁写清单（施工 5-17 下，2026-10-22 项目主人定，照各家对照 `私密目录-各家对照.md`「只禁写」的推荐）：改了以后会在
//! 沙盒外被执行的几样。前七个是相对路径，指每个工作区、加进来的目录里任一层的那一处，三个平台一样、先后不变；后面是家目录下的
//! shell 启动文件、git 的全局配置、自启动，写成 `~/…`，照系统不同。不管在不在。

/// 工作区里的：git 下次跑的钩子和配置，agent 自己的配置，编辑器的任务，direnv 进目录就跑的 `.envrc`。
const INSIDE: [&str; 7] = [
    ".git/hooks",
    ".git/config",
    ".miyu",
    ".mcp.json",
    ".vscode",
    ".idea",
    ".envrc",
];

/// Linux：shell 启动文件、git 的全局配置、systemd 的用户服务、桌面的自启动。
#[cfg(target_os = "linux")]
pub const SHIPPED_READONLY: &[&str] = &[
    INSIDE[0],
    INSIDE[1],
    INSIDE[2],
    INSIDE[3],
    INSIDE[4],
    INSIDE[5],
    INSIDE[6],
    "~/.bashrc",
    "~/.bash_profile",
    "~/.bash_login",
    "~/.profile",
    "~/.zshrc",
    "~/.zshenv",
    "~/.zprofile",
    "~/.zlogin",
    "~/.config/fish",
    "~/.gitconfig",
    "~/.config/git/config",
    "~/.config/systemd/user",
    "~/.config/autostart",
];

/// macOS：shell 启动文件、git 的全局配置、登录时自启动的 `LaunchAgents`。
#[cfg(target_os = "macos")]
pub const SHIPPED_READONLY: &[&str] = &[
    INSIDE[0],
    INSIDE[1],
    INSIDE[2],
    INSIDE[3],
    INSIDE[4],
    INSIDE[5],
    INSIDE[6],
    "~/.bashrc",
    "~/.bash_profile",
    "~/.bash_login",
    "~/.profile",
    "~/.zshrc",
    "~/.zshenv",
    "~/.zprofile",
    "~/.zlogin",
    "~/.config/fish",
    "~/.gitconfig",
    "~/.config/git/config",
    "~/Library/LaunchAgents",
];

/// Windows：git 的全局配置、PowerShell 的配置文件（新版、5.1 两处，各两份），用途同 shell 启动文件。
#[cfg(windows)]
pub const SHIPPED_READONLY: &[&str] = &[
    INSIDE[0],
    INSIDE[1],
    INSIDE[2],
    INSIDE[3],
    INSIDE[4],
    INSIDE[5],
    INSIDE[6],
    "~/.gitconfig",
    "~/.config/git/config",
    "~/Documents/PowerShell/Microsoft.PowerShell_profile.ps1",
    "~/Documents/PowerShell/profile.ps1",
    "~/Documents/WindowsPowerShell/Microsoft.PowerShell_profile.ps1",
    "~/Documents/WindowsPowerShell/profile.ps1",
];

/// 别的 Unix：shell 启动文件、git 的全局配置。
#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
pub const SHIPPED_READONLY: &[&str] = &[
    INSIDE[0],
    INSIDE[1],
    INSIDE[2],
    INSIDE[3],
    INSIDE[4],
    INSIDE[5],
    INSIDE[6],
    "~/.bashrc",
    "~/.bash_profile",
    "~/.bash_login",
    "~/.profile",
    "~/.zshrc",
    "~/.zshenv",
    "~/.zprofile",
    "~/.zlogin",
    "~/.config/fish",
    "~/.gitconfig",
    "~/.config/git/config",
];
