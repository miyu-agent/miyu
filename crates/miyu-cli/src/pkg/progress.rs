//! 装包时照核心推的 `package.progress` 印在标准错误上（施工 IN-3，设计 `49-装包照pacman.md` 第二节第 4 条，照 pacman）：
//!
//! - 下载：第一次推来时印「正在下载…」。标准错误是终端的，每个包一行进度条，照推来的刷新同一行（`\r`），下完换行；不是终端的
//!   不画，下完一个印一行「已下载 <编号>-<版本> <大小>」。没写 `stage` 的（老的核心）当下载。
//! - 核对：「(i/N) 正在核对完整性」，终端里刷新同一行，只留最后那个数。
//! - 安装：「(i/N) 正在安装 <编号>」，一个一行。
//!
//! 一行进度条怎么排的 [`bar`] 是纯的，测试里照它算。

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

use serde_json::Value;

use super::query::size;
use crate::config::columns;
use crate::language::Language;
use crate::shown::{say, write};

/// 量不到终端多宽的当多宽。
pub(crate) const FALLBACK_COLUMNS: usize = 80;

/// 进度条至少多宽：再窄的不画条，只印名字、大小、百分比。
const NARROWEST: usize = 10;

/// 大小那一格多宽（右对齐）：最宽的是「1023.9 KiB」。
const SIZE: usize = 10;

/// 擦掉这一行、回到行首。
const REDRAW: &str = "\r\x1b[2K";

/// 标准错误是不是终端：是的交回它多宽（量不到的当 [`FALLBACK_COLUMNS`]），不是的没有。
pub(crate) fn terminal() -> Option<usize> {
    use std::io::IsTerminal;
    let err = std::io::stderr();
    err.is_terminal()
        .then(|| width(&err).unwrap_or(FALLBACK_COLUMNS))
}

#[cfg(unix)]
fn width(err: &std::io::Stderr) -> Option<usize> {
    rustix::termios::tcgetwinsize(err)
        .ok()
        .map(|size| usize::from(size.ws_col))
        .filter(|columns| *columns > 0)
}

#[cfg(not(unix))]
fn width(_: &std::io::Stderr) -> Option<usize> {
    None
}

/// 一行进度条：` <名字补到 name_width> <大小右对齐>  [####----] 百分比`，整行不超过 `columns - 1`（最后一格留着，免得终端
/// 自己折行）；放不下 [`NARROWEST`] 格的条不画，再放不下的从后面截掉。
pub(super) fn bar(
    (name, name_width): (&str, usize),
    (done, total): (u64, u64),
    columns: usize,
) -> String {
    let done = done.min(total);
    let percent = done.saturating_mul(100).checked_div(total).unwrap_or(100);
    let pad = " ".repeat(name_width.saturating_sub(self::columns(name)));
    let head = format!(" {name}{pad} {:>SIZE$}", size(total));
    let tail = format!(" {percent:>3}%");
    let most = columns.saturating_sub(1);
    let room = most.saturating_sub(self::columns(&head) + 2 + 2 + self::columns(&tail));
    if room < NARROWEST {
        return fit(&format!("{head}{tail}"), most);
    }
    let filled = match total {
        0 => room,
        _ => {
            let wide = u128::try_from(room).unwrap_or(u128::MAX);
            let filled = wide.saturating_mul(u128::from(done)) / u128::from(total);
            usize::try_from(filled).unwrap_or(room).min(room)
        }
    };
    format!(
        "{head}  [{}{}]{tail}",
        "#".repeat(filled),
        "-".repeat(room - filled)
    )
}

/// `line` 截到显示宽度不超过 `most`。
fn fit(line: &str, most: usize) -> String {
    let mut used = 0;
    line.chars()
        .take_while(|c| {
            used += columns(c.encode_utf8(&mut [0; 4]));
            used <= most
        })
        .collect()
}

/// 画到一半的那一行是什么。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Open {
    /// 包 `id` 的进度条。
    Bar(String),
    /// 核对那一行。
    Verify,
}

/// 一次 `package.install` 的进度怎么印。
pub(super) struct Progress<'a> {
    language: &'a Language,
    /// 标准错误是终端的：多宽。
    terminal: Option<usize>,
    /// 编号到「<编号>-<版本>」。
    names: BTreeMap<String, String>,
    /// 名字那一格多宽：这一次要装的里最宽的。
    width: usize,
    /// 印过「正在下载…」了。
    downloading: bool,
    /// 画到一半、还没换行的那一行（只有终端里有）。
    open: Option<Open>,
    /// 不在终端里：印过「已下载」的。
    downloaded: BTreeSet<String>,
}

impl<'a> Progress<'a> {
    /// 这一次要装的 `packages`（编号、版本，照 [`super::summary::packages`]）；`terminal` 是标准错误多宽（不是终端的没有）。
    pub(super) fn new(
        language: &'a Language,
        terminal: Option<usize>,
        packages: &[(String, String)],
    ) -> Progress<'a> {
        let names: BTreeMap<String, String> = packages
            .iter()
            .map(|one| (one.0.clone(), super::summary::named(one)))
            .collect();
        let width = names.values().map(|name| columns(name)).max().unwrap_or(0);
        Progress {
            language,
            terminal,
            names,
            width,
            downloading: false,
            open: None,
            downloaded: BTreeSet::new(),
        }
    }

    /// 核心推来的一条：不是 `package.progress` 的不管。
    pub(super) fn on(&mut self, message: &Value, err: &mut dyn Write) {
        if message["method"] != "package.progress" {
            return;
        }
        let params = &message["params"];
        let id = params["package"].as_str().unwrap_or_default();
        let number = |key: &str| params[key].as_u64().unwrap_or(1);
        let (index, count) = (number("index"), number("count"));
        match params["stage"].as_str() {
            Some("verify") => {
                let line = self.language.checking_integrity(index, count);
                self.line(err, &line, Some(Open::Verify));
            }
            Some("install") => {
                let line = self.language.installing_one(index, count, id);
                self.line(err, &line, None);
            }
            _ => {
                let done = params["done"].as_u64().unwrap_or_default();
                let total = params["total"].as_u64().unwrap_or_default();
                self.download(err, id, (done, total));
            }
        }
    }

    /// 收尾：画到一半的那一行换行。
    pub(super) fn finish(&mut self, err: &mut dyn Write) {
        if self.open.take().is_some() {
            write(err, "\n");
        }
    }

    /// 印一行：终端里 `keep` 的那一种刷新同一行、先不换行；别的（和不在终端里的）一行一个。
    fn line(&mut self, err: &mut dyn Write, line: &str, keep: Option<Open>) {
        match (self.terminal, keep) {
            (Some(_), Some(keep)) => {
                if self.open.as_ref() != Some(&keep) {
                    self.finish(err);
                }
                write(err, &format!("{REDRAW}{line}"));
                self.open = Some(keep);
            }
            _ => {
                self.finish(err);
                say(err, line);
            }
        }
    }

    /// 下载 `id`：下了多少、一共多少。
    fn download(&mut self, err: &mut dyn Write, id: &str, (done, total): (u64, u64)) {
        if !self.downloading {
            self.downloading = true;
            self.finish(err);
            say(err, self.language.downloading());
        }
        let name = self.names.get(id).map_or(id, String::as_str).to_string();
        let ended = done >= total;
        match self.terminal {
            Some(wide) => {
                let open = Open::Bar(id.to_string());
                if self.open.as_ref() != Some(&open) {
                    self.finish(err);
                }
                let width = self.width.max(columns(&name));
                let line = bar((&name, width), (done, total), wide);
                write(err, &format!("{REDRAW}{line}"));
                self.open = Some(open);
                if ended {
                    self.finish(err);
                }
            }
            None => {
                if ended && self.downloaded.insert(id.to_string()) {
                    say(err, &self.language.downloaded(&name, &size(total)));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
