//! Windows 的路径照字面换成规整的写法再判（施工 WN-1）：不分大小写（一律小写），`\`、`/` 都是分隔，盘符当第一段——
//! `C:\Users\me` 写成 `/c:/users/me`，再交给和 bash 那一路同一个判法（`paths::judge`，盘符本身算根目录，盘符下一级算系统
//! 目录）。纯逻辑，不碰磁盘。
//!
//! 接家目录的：`~`、`$HOME`、`$env:USERPROFILE`（连同 `${…}` 的写法）、cmd 的 `%USERPROFILE%`；`$env:SystemDrive`、
//! `%SystemDrive%` 照家目录的盘；`$env:SystemRoot`、`$env:windir` 照家目录的盘下的 `windows`。`\x` 照当前目录的盘。
//! 别的变量说不清；网络路径（`\\server\share`）、注册表这类别的提供程序（`HKLM:\…`）不是这台机器的盘，不判。

use super::super::lex::Word;
use super::super::paths::judge;
use super::super::{Kind, Places};

/// 规整过的 [`Places`]：都照 [`tidy`] 写。
pub(super) struct Tidy {
    pub(super) cwd: String,
    home: Option<String>,
    data_root: String,
    workspaces: String,
}

impl Tidy {
    /// 照调用带的几样写（家目录、数据根照这台机器的写法，`\\?\` 开头的也认）；换不成的照原样小写。
    pub(super) fn of(places: &Places<'_>) -> Tidy {
        let tidied = |text: &str| tidy(text).unwrap_or_else(|| text.to_lowercase());
        Tidy {
            cwd: tidied(places.cwd),
            home: places.home.and_then(tidy),
            data_root: tidied(places.data_root),
            workspaces: tidied(places.workspaces),
        }
    }

    /// 交给判法的样子。
    pub(super) fn places(&self) -> Places<'_> {
        Places {
            cwd: &self.cwd,
            home: self.home.as_deref(),
            data_root: &self.data_root,
            workspaces: &self.workspaces,
        }
    }

    /// 系统盘：照家目录的盘（`/c:`）。
    fn drive(&self) -> Option<String> {
        let home = self.home.as_deref()?;
        home.split('/').nth(1).map(|drive| format!("/{drive}"))
    }
}

/// 一个目标落在哪。
pub(super) enum Where {
    /// 这台机器的盘上，规整过的写法。
    Path(String),
    /// 要到运行时才知道。
    Unknown,
    /// 不是这台机器的盘（网络路径、注册表这类别的提供程序）：不判。
    Elsewhere,
}

/// 删一个目标：落在盘上的照判法判，说不清的整个目录往下删才问，不是盘上的不判。
pub(super) fn target(word: &Word, tidy: &Tidy, cwd: Option<&str>, recursive: bool) -> Option<Kind> {
    match absolute(word, tidy, cwd) {
        Where::Path(path) => judge(&path, &tidy.places(), true),
        Where::Unknown => recursive.then_some(Kind::DeleteUnresolved),
        Where::Elsewhere => None,
    }
}

/// 照字面换成规整的绝对路径：认得的变量照家目录换，盘符开头的照原样，`\` 开头的接当前目录的盘，相对的接 `cwd`（空的是
/// 说不清）。
pub(super) fn absolute(word: &Word, tidy: &Tidy, cwd: Option<&str>) -> Where {
    let lower = word.text.to_lowercase();
    // 扩展长度的写法（`\\?\C:\x`）就是后面那一截。
    let text = lower
        .strip_prefix(r"\\?\")
        .or_else(|| lower.strip_prefix("//?/"))
        .unwrap_or(&lower);
    if let Some((base, rest)) = known(text, tidy) {
        return match base {
            Some(base) if !rest.contains(['$', '%']) => Where::Path(join(&base, rest)),
            _ => Where::Unknown,
        };
    }
    if word.unresolved {
        return Where::Unknown;
    }
    let slash = |c: char| c == '\\' || c == '/';
    let mut chars = text.chars();
    let (first, second, third) = (chars.next(), chars.next(), chars.next());
    match (first, second, third) {
        (Some(a), Some(b), _) if slash(a) && slash(b) => Where::Elsewhere,
        (Some(a), _, _) if slash(a) => match cwd.and_then(|cwd| cwd.split('/').nth(1)) {
            Some(drive) => Where::Path(join(&format!("/{drive}"), text)),
            None => Where::Unknown,
        },
        (Some(letter), Some(':'), Some(c)) if letter.is_ascii_alphabetic() && slash(c) => {
            Where::Path(join(&format!("/{}", &text[..2]), &text[2..]))
        }
        // 只有盘符、盘符后面接相对路径的（`C:`、`C:x`）照那个盘自己的当前目录，说不清。
        (Some(letter), Some(':'), _) if letter.is_ascii_alphabetic() => Where::Unknown,
        _ if text
            .split(['\\', '/'])
            .next()
            .is_some_and(|first| first.contains(':')) =>
        {
            Where::Elsewhere
        }
        _ => match cwd {
            Some(cwd) => Where::Path(join(cwd, text)),
            None => Where::Unknown,
        },
    }
}

/// 认得的开头（已经小写）：接到哪（家目录不知道的是空的）、后面那一截。只认紧跟着结尾或分隔符的。
fn known<'a>(text: &'a str, tidy: &Tidy) -> Option<(Option<String>, &'a str)> {
    const HOME: [&str; 7] = [
        "~",
        "$home",
        "${home}",
        "$env:userprofile",
        "${env:userprofile}",
        "%userprofile%",
        "%homedrive%%homepath%",
    ];
    const DRIVE: [&str; 6] = [
        "$env:systemdrive",
        "${env:systemdrive}",
        "$env:homedrive",
        "%systemdrive%",
        "%homedrive%",
        "${env:homedrive}",
    ];
    const WINDOWS: [&str; 6] = [
        "$env:systemroot",
        "${env:systemroot}",
        "$env:windir",
        "${env:windir}",
        "%systemroot%",
        "%windir%",
    ];
    let after = |prefix: &str| {
        text.strip_prefix(prefix)
            .filter(|rest| rest.is_empty() || rest.starts_with(['\\', '/']))
    };
    if let Some(rest) = HOME.iter().find_map(|prefix| after(prefix)) {
        return Some((tidy.home.clone(), rest));
    }
    if let Some(rest) = DRIVE.iter().find_map(|prefix| after(prefix)) {
        return Some((tidy.drive(), rest));
    }
    WINDOWS
        .iter()
        .find_map(|prefix| after(prefix))
        .map(|rest| (tidy.drive().map(|drive| format!("{drive}/windows")), rest))
}

/// 这台机器写法的绝对路径（`C:\x`、`\\?\C:\x`）规整成 `/c:/x`；不是盘符开头的绝对路径交回空的。
pub(super) fn tidy(text: &str) -> Option<String> {
    let text = text.to_lowercase().replace('\\', "/");
    let text = text.strip_prefix("//?/").unwrap_or(&text);
    let mut chars = text.chars();
    let (letter, colon, slash) = (chars.next()?, chars.next()?, chars.next());
    if !letter.is_ascii_alphabetic() || colon != ':' || !matches!(slash, None | Some('/')) {
        return None;
    }
    Some(join(&format!("/{}", &text[..2]), &text[2..]))
}

/// 在规整过的 `base`（`/c:/x`）后面接上 `rest`，`.`、`..` 照字面去掉，`..` 退不过盘符。
fn join(base: &str, rest: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in base.split('/').chain(rest.split(['\\', '/'])) {
        match part {
            "" | "." => {}
            ".." => {
                if parts.len() > 1 {
                    parts.pop();
                }
            }
            part => parts.push(part),
        }
    }
    format!("/{}", parts.join("/"))
}
