//! 照哪个服务管理器、`miyu` 在哪、数据根在哪，算出要写的文件和要跑的命令（施工 Z-1 下）：只算不做，测试直接对它。文件的样子
//! 照仓库 `packaging/` 里那几份（同一份，不另写），换上 `miyu` 的真实位置；设了 `MIYU_HOME` 的一并写上，服务起来用的是同一个
//! 数据根。

use std::path::{Path, PathBuf};

/// 用户级的服务管理器。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Manager {
    /// Linux 上有 systemd 用户会话的：用户服务。
    Systemd,
    /// Linux 上没有 systemd 用户会话的桌面：XDG 自启动。
    Autostart,
    /// macOS：launchd 的用户代理。
    Launchd,
    /// Windows：登录时启动的任务计划。
    Task,
}

impl Manager {
    /// 机读格式里的写法（`cli/service.md`「机读格式」）。
    pub(crate) fn code(self) -> &'static str {
        match self {
            Manager::Systemd => "systemd",
            Manager::Autostart => "autostart",
            Manager::Launchd => "launchd",
            Manager::Task => "task",
        }
    }
}

/// 这个人的几个目录和身份：文件放哪、命令带什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Place {
    /// 家目录。
    pub(crate) home: PathBuf,
    /// `XDG_CONFIG_HOME`（Linux）：没设的照 `~/.config`。
    pub(crate) config: Option<PathBuf>,
    /// 用户编号（macOS 的 `gui/<uid>`）。
    pub(crate) uid: Option<u32>,
    /// Windows 的 `<域>\<用户名>`：任务只在这个人登录时起。
    pub(crate) user: Option<String>,
}

/// 要写的那一个文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Unit {
    /// 写在哪。
    pub(crate) file: PathBuf,
    /// 写什么。
    pub(crate) text: String,
}

/// 算不出来的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Problem {
    /// Windows 的任务计划带不了环境变量：设了 `MIYU_HOME` 的装不了。
    DataRootOnWindows,
}

/// launchd 用户代理的名字（同 `packaging/macos/miyu.plist` 的 `Label`）。
pub(crate) const LABEL: &str = "io.github.miyu-agent.core";

/// 任务计划的名字。
pub(crate) const TASK: &str = "Miyu";

/// 样子：仓库 `packaging/` 里的那几份（施工 Z-1 上），只认这一份。
const SYSTEMD: &str = include_str!("../../../../packaging/systemd/user/miyu.service");
const DESKTOP: &str = include_str!("../../../../packaging/xdg/miyu.desktop");
const PLIST: &str = include_str!("../../../../packaging/macos/miyu.plist");
const TASK_XML: &str = include_str!("../../../../packaging/windows/miyu.xml");

/// 照管理器 `manager` 写的那一个文件：跑 `exe core`，数据根照 `data_root`（设了 `MIYU_HOME` 的），放在 `place` 下面。
///
/// # Errors
///
/// [`Problem`]。
pub(crate) fn unit(
    manager: Manager,
    exe: &Path,
    data_root: Option<&Path>,
    place: &Place,
) -> Result<Unit, Problem> {
    let config = place
        .config
        .clone()
        .unwrap_or_else(|| place.home.join(".config"));
    let exe_text = exe.to_string_lossy();
    let (file, text) = match manager {
        Manager::Systemd => {
            let mut text = String::new();
            for line in SYSTEMD.lines() {
                if line.starts_with("ExecStart=") {
                    text.push_str(&format!("ExecStart={} core\n", systemd_word(&exe_text)));
                    continue;
                }
                text.push_str(line);
                text.push('\n');
                if let (true, Some(root)) = (line == "[Service]", data_root) {
                    let pair = format!("MIYU_HOME={}", root.to_string_lossy());
                    text.push_str(&format!("Environment={}\n", systemd_word(&pair)));
                }
            }
            (config.join("systemd/user/miyu.service"), text)
        }
        Manager::Autostart => {
            let env = data_root.map_or_else(String::new, |root| {
                format!("env MIYU_HOME={} ", desktop_word(&root.to_string_lossy()))
            });
            let exec = format!("Exec={env}{} core", desktop_word(&exe_text));
            let text = replace_line(DESKTOP, "Exec=", &exec);
            (config.join("autostart/miyu.desktop"), text)
        }
        Manager::Launchd => {
            let mut text = PLIST.replace(
                "<string>/opt/homebrew/bin/miyu</string>",
                &format!("<string>{}</string>", xml(&exe_text)),
            );
            if let Some(root) = data_root {
                let env = format!(
                    "  <key>EnvironmentVariables</key>\n  <dict>\n    <key>MIYU_HOME</key>\n    <string>{}</string>\n  </dict>\n  <key>RunAtLoad</key>",
                    xml(&root.to_string_lossy())
                );
                text = text.replacen("  <key>RunAtLoad</key>", &env, 1);
            }
            let file = place
                .home
                .join("Library/LaunchAgents")
                .join(format!("{LABEL}.plist"));
            (file, text)
        }
        Manager::Task => {
            if data_root.is_some() {
                return Err(Problem::DataRootOnWindows);
            }
            let mut text = TASK_XML.replace(
                r"<Command>%LOCALAPPDATA%\Programs\Miyu\miyu.exe</Command>",
                &format!("<Command>{}</Command>", xml(&exe_text)),
            );
            if let Some(user) = &place.user {
                text = text.replacen(
                    "      <Enabled>true</Enabled>\n",
                    &format!(
                        "      <Enabled>true</Enabled>\n      <UserId>{}</UserId>\n",
                        xml(user)
                    ),
                    1,
                );
            }
            (place.home.join(r"AppData\Local\Miyu\miyu-task.xml"), text)
        }
    };
    Ok(Unit { file, text })
}

/// 照 `prefix` 开头的那一行换成 `line`，别的原样。
fn replace_line(text: &str, prefix: &str, line: &str) -> String {
    text.lines()
        .map(|each| if each.starts_with(prefix) { line } else { each })
        .map(|each| format!("{each}\n"))
        .collect()
}

/// systemd 单元里的一个词：`%` 写两个；有空白、引号、反斜杠的加双引号、里面的转义。
fn systemd_word(word: &str) -> String {
    let word = word.replace('%', "%%");
    if !word
        .chars()
        .any(|c| c.is_whitespace() || c == '"' || c == '\\' || c == '\'')
    {
        return word;
    }
    format!("\"{}\"", word.replace('\\', "\\\\").replace('"', "\\\""))
}

/// `.desktop` 的 `Exec` 里的一个词（XDG 桌面条目规范）：`%` 写两个；有保留字符的加双引号，`"`、`` ` ``、`$`、`\` 前面加反斜杠。
fn desktop_word(word: &str) -> String {
    let word = word.replace('%', "%%");
    let reserved = |c: char| c.is_whitespace() || "\"'\\><~|&;$*?#()`".contains(c);
    if !word.chars().any(reserved) {
        return word;
    }
    let mut quoted = String::from("\"");
    for c in word.chars() {
        if "\"`$\\".contains(c) {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('"');
    quoted
}

/// XML 的字：`&`、`<`、`>`、引号转义。
fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 一条命令。
fn command(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_string()).collect()
}

/// systemd 用户服务的名字。
const SERVICE: &str = "miyu.service";

/// 装好以后要跑的命令：一条一个程序加参数。自启动的不用跑：下次登录桌面时起。
pub(crate) fn enable(manager: Manager, unit: &Unit, place: &Place) -> Vec<Vec<String>> {
    let file = unit.file.to_string_lossy();
    match manager {
        Manager::Systemd => vec![
            command(&["systemctl", "--user", "daemon-reload"]),
            command(&["systemctl", "--user", "enable", "--now", SERVICE]),
        ],
        Manager::Autostart => Vec::new(),
        Manager::Launchd => vec![command(&["launchctl", "bootstrap", &gui(place), &file])],
        Manager::Task => vec![command(&[
            "schtasks", "/Create", "/TN", TASK, "/XML", &file, "/F",
        ])],
    }
}

/// 卸之前要跑的命令。
pub(crate) fn disable(manager: Manager, place: &Place) -> Vec<Vec<String>> {
    match manager {
        Manager::Systemd => vec![command(&[
            "systemctl",
            "--user",
            "disable",
            "--now",
            SERVICE,
        ])],
        Manager::Autostart => Vec::new(),
        Manager::Launchd => vec![command(&[
            "launchctl",
            "bootout",
            &format!("{}/{LABEL}", gui(place)),
        ])],
        Manager::Task => vec![command(&["schtasks", "/Delete", "/TN", TASK, "/F"])],
    }
}

/// 问服务管理器认不认得、开没开着的命令：退出码 0 是开着（systemd 是在跑，launchd、任务计划是登记着）。自启动的问不了，没有。
pub(crate) fn running(manager: Manager, place: &Place) -> Option<Vec<String>> {
    match manager {
        Manager::Systemd => Some(command(&[
            "systemctl",
            "--user",
            "is-active",
            "--quiet",
            SERVICE,
        ])),
        Manager::Autostart => None,
        Manager::Launchd => Some(command(&[
            "launchctl",
            "print",
            &format!("{}/{LABEL}", gui(place)),
        ])),
        Manager::Task => Some(command(&["schtasks", "/Query", "/TN", TASK])),
    }
}

/// launchd 的这个人那一域：`gui/<uid>`。
fn gui(place: &Place) -> String {
    format!("gui/{}", place.uid.unwrap_or_default())
}

#[cfg(test)]
mod tests;
