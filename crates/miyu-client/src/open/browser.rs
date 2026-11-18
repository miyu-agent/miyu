//! 用系统的办法把网址交给浏览器（`web-ui.md`「怎么走」第二条第 4 款）。

use std::process::Command;

use super::{Browser, HAND_OVER, handed};

/// 系统的办法：Linux 是 `xdg-open`，macOS 是 `open`，Windows 经 `rundll32` 交给默认浏览器（施工 AU-014，同 `miyu mcp login`）。
pub struct SystemBrowser;

impl Browser for SystemBrowser {
    fn open(&self, url: &str) -> bool {
        handed(opener(url), HAND_OVER)
    }
}

/// 打开 `url` 的那条命令：网址整个是一个参数，不经过任何 shell。Windows 上不用 `cmd /C start "" <网址>`（审计 #014）：`cmd`
/// 照自己的规矩再读一遍这一行，网址里的 `&`、`|` 是命令分隔符、`%名字%` 会被换掉，多个参数的网址在 `&` 处断开，写得坏的能另起
/// 命令。
fn opener(url: &str) -> Command {
    let mut command = if cfg!(target_os = "macos") {
        Command::new("open")
    } else if cfg!(windows) {
        let mut command = Command::new("rundll32.exe");
        command.arg("url.dll,FileProtocolHandler");
        command
    } else {
        Command::new("xdg-open")
    };
    command.arg(url);
    command
}

#[cfg(test)]
mod tests;
