//! PowerShell 的高危命令（施工 WN-1，2026-11-14 项目主人定发版前补）：Windows 上她的命令走 PowerShell（`shell` 的
//! `program.rs`），照 bash 那一路的口径认同样的几种，拒的照拒、问的照问，归同样的类（设置页「受保护的命令」五个开关在
//! Windows 上照样管用）。cmd 的写法只在 `cmd /c "…"` 里出现，拆出来照同一套规则认（`cmd.rs`）。
//!
//! 认的（同 bash 那一路的哪一种）：
//! - 删除：`Remove-Item` 和别名、cmd 的 `rd`、`del`（`delete.rs`），目标照 Windows 的写法判（`paths.rs`）：删盘符根、家目录、
//!   盘符下一级（`C:\Windows`、`C:\Users`）拒，删数据根、工作目录、目标说不清的整个目录往下删问；挪走的 `Move-Item`、
//!   `Rename-Item` 和 cmd 的 `move`、`ren` 只管数据根（施工 RO-4、RO-4补）；
//! - git、容器：和 bash 那一路同一份规则（`git.exe`、`docker.exe` 照名字认）；
//! - 格式化：`Format-Volume`、`Clear-Disk`、`Remove-Partition`、cmd 的 `format <盘符>:`，拒；
//! - 关机、重启：`Stop-Computer`、`Restart-Computer`、`shutdown`（`/a` 取消关机的不算），问；
//! - 下载下来直接执行：`irm … | iex`、`iex (irm …)`、`iex (New-Object Net.WebClient).DownloadString(…)`，接到 `powershell`、
//!   `pwsh` 的标准输入的也算，问；
//! - 外壳：`Invoke-Expression`（`iex`）、`powershell -Command`、`pwsh -c`、`cmd /c`、`$( )`、括号、脚本块里的另认，套得太深的
//!   照问（[`Kind::TooDeep`]）；单独成句的 `cd`、`Set-Location` 跟着换当前目录。
//!
//! 不认的：变量里存的路径（bash 那一路的 `vars.rs` 没有对应）、`-EncodedCommand`、往裸设备写（`\\.\PhysicalDrive0`）。

mod cmd;
mod delete;
mod lex;
mod paths;

use super::container::container;
use super::lex::{Simple, Word};
use super::rules::{Assessed, DEPTH, git, settle};
use super::{Kind, Places};
use paths::{Tidy, Where, absolute};

/// 一段原文照哪种 shell 拆。
#[derive(Clone, Copy)]
enum Shell {
    PowerShell,
    Cmd,
}

/// 认一条 PowerShell 命令 `command`：判下来的；什么都没认出来的是空的。`places` 照这台机器的写法（`C:\Users\me`）。
pub fn assess_powershell(command: &str, places: &Places<'_>) -> Option<Assessed> {
    let tidy = Tidy::of(places);
    let mut found = Vec::new();
    look(command, Shell::PowerShell, &tidy, 0, &mut found);
    settle(found)
}

fn look(text: &str, shell: Shell, tidy: &Tidy, depth: usize, found: &mut Vec<Kind>) {
    if depth > DEPTH {
        found.push(Kind::TooDeep);
        return;
    }
    let parsed = match shell {
        Shell::PowerShell => lex::parse(text),
        Shell::Cmd => cmd::parse(text),
    };
    let mut cwd = Some(tidy.cwd.clone());
    for pipeline in &parsed.pipelines {
        if fetch_into_shell(pipeline) {
            found.push(Kind::PipeToShell);
        }
        for (at, simple) in pipeline.iter().enumerate() {
            one(simple, &pipeline[..at], tidy, cwd.as_deref(), depth, found);
        }
        if let Some(next) = moved(pipeline, tidy, cwd.as_deref()) {
            cwd = next;
        }
    }
    if fetched_then_run(&parsed) {
        found.push(Kind::PipeToShell);
    }
    for inner in &parsed.inner {
        look(inner, Shell::PowerShell, tidy, depth + 1, found);
    }
}

/// 不在一条管道里的下载后执行：`iex (irm …)`、`iex (New-Object Net.WebClient).DownloadString(…)`、`$s = irm …; iex $s`
/// （有 `iex`、哪里在下载），`powershell -c (irm …)`（shell 的参数里在下载）。
fn fetched_then_run(parsed: &lex::Parsed) -> bool {
    let simples: Vec<&Simple> = parsed.pipelines.iter().flatten().collect();
    let named = |pick: fn(&str) -> bool| {
        simples.iter().any(|simple| {
            words(&simple.words)
                .first()
                .is_some_and(|word| pick(&base(&word.text)))
        })
    };
    let inner = parsed.inner.iter().any(|text| fetches(text));
    let here = named(fetcher)
        || simples
            .iter()
            .any(|simple| simple.words.iter().any(|word| downloads(&word.text)));
    (named(evaluates) && (here || inner)) || (named(runs) && inner)
}

/// 一个简单命令：`fed` 是管道里排在它前面的那几个。
fn one(
    simple: &Simple,
    fed: &[Simple],
    tidy: &Tidy,
    cwd: Option<&str>,
    depth: usize,
    found: &mut Vec<Kind>,
) {
    let Some((first, rest)) = words(&simple.words).split_first() else {
        return;
    };
    let name = base(&first.text);
    let lower: Vec<String> = rest.iter().map(|word| word.text.to_lowercase()).collect();
    let after = |at: usize| -> String {
        let script: Vec<&str> = rest[at + 1..]
            .iter()
            .map(|word| word.text.as_str())
            .collect();
        script.join(" ")
    };
    match name.as_str() {
        "cmd" => {
            if let Some(at) = lower
                .iter()
                .position(|arg| matches!(arg.as_str(), "/c" | "/k" | "/r"))
            {
                look(&after(at), Shell::Cmd, tidy, depth + 1, found);
            }
        }
        "powershell" | "pwsh" => {
            if let Some(at) = lower.iter().position(|arg| {
                arg.strip_prefix('-')
                    .is_some_and(|name| !name.is_empty() && "command".starts_with(name))
            }) {
                look(&after(at), Shell::PowerShell, tidy, depth + 1, found);
            }
        }
        "invoke-expression" | "iex" => {
            let script: Vec<&str> = rest.iter().map(|word| word.text.as_str()).collect();
            look(&script.join(" "), Shell::PowerShell, tidy, depth + 1, found);
        }
        name if delete::deletes(name) => delete::deleting(rest, fed, tidy, cwd, found),
        name if delete::moves(name) => delete::moving(rest, tidy, cwd, found),
        "git" => git(rest, found),
        "format-volume" | "clear-disk" | "remove-partition" => found.push(Kind::DiskFormat),
        "format" if lower.iter().any(|arg| drive_letter(arg)) => found.push(Kind::DiskFormat),
        "stop-computer" | "restart-computer" => found.push(Kind::Power),
        "shutdown"
            if !lower
                .iter()
                .any(|arg| matches!(arg.as_str(), "/a" | "-a" | "/?")) =>
        {
            found.push(Kind::Power);
        }
        name => found.extend(container(name, rest)),
    }
}

/// 只有盘符的参数：`C:`、`C:\`、`D:/`。
fn drive_letter(arg: &str) -> bool {
    let mut chars = arg.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.next() == Some(':')
        && matches!(chars.next(), None | Some('\\' | '/'))
        && chars.next().is_none()
}

/// 剥掉开头的：调用运算符 `.`、赋值（`$x =`）、Windows 自带的 `sudo`。
pub(super) fn words(words: &[Word]) -> &[Word] {
    let mut at = 0;
    loop {
        match words.get(at).map(|word| word.text.as_str()) {
            Some("." | "sudo") => at += 1,
            Some(text)
                if text.starts_with('$')
                    && words
                        .get(at + 1)
                        .is_some_and(|next| matches!(next.text.as_str(), "=" | "+=")) =>
            {
                at += 2;
            }
            _ => return words.get(at..).unwrap_or_default(),
        }
    }
}

/// 程序名：路径的最后一段，小写，去掉 `.exe`（`C:\Windows\System32\shutdown.exe` 是 `shutdown`）。
pub(super) fn base(text: &str) -> String {
    let last = text
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(text)
        .to_lowercase();
    match last.strip_suffix(".exe") {
        Some(name) => name.to_string(),
        None => last,
    }
}

/// 单独成句的 `cd`、`Set-Location`、`Push-Location`：换到哪（`-`、说不清的、不写的是空的）；`popd` 照说不清；不是的交回空的。
fn moved(pipeline: &[Simple], tidy: &Tidy, cwd: Option<&str>) -> Option<Option<String>> {
    let [simple] = pipeline else {
        return None;
    };
    let (first, rest) = words(&simple.words).split_first()?;
    match base(&first.text).as_str() {
        "cd" | "chdir" | "set-location" | "sl" | "pushd" | "push-location" => {}
        "popd" | "pop-location" => return Some(None),
        _ => return None,
    }
    let dir = rest
        .iter()
        .find(|word| !word.text.starts_with('-') && !word.text.eq_ignore_ascii_case("/d"));
    Some(match dir {
        Some(word) if word.text != "-" => match absolute(word, tidy, cwd) {
            Where::Path(path) => Some(path),
            Where::Unknown | Where::Elsewhere => None,
        },
        _ => None,
    })
}

/// 下载的命令。
fn fetcher(name: &str) -> bool {
    matches!(
        name,
        "invoke-webrequest" | "iwr" | "invoke-restmethod" | "irm" | "curl" | "wget"
    )
}

/// `Invoke-Expression`：把一段字当 PowerShell 代码跑。
fn evaluates(name: &str) -> bool {
    matches!(name, "invoke-expression" | "iex")
}

/// 拿一段字当代码跑的：`Invoke-Expression`，读标准输入的 shell、解释器。
fn runs(name: &str) -> bool {
    evaluates(name)
        || matches!(
            name,
            "powershell" | "pwsh" | "cmd" | "sh" | "bash" | "python" | "python3" | "node"
        )
}

/// 一条管道里先下载、后面接着 `iex`，或者接着不带脚本参数的 shell、解释器：`irm … | iex`。
fn fetch_into_shell(pipeline: &[Simple]) -> bool {
    let mut fetched = false;
    for simple in pipeline {
        let Some((first, rest)) = words(&simple.words).split_first() else {
            continue;
        };
        let name = base(&first.text);
        if fetcher(&name) {
            fetched = true;
        } else if fetched
            && runs(&name)
            && (evaluates(&name)
                || rest
                    .iter()
                    .all(|word| word.text.starts_with('-') || word.text.starts_with('/')))
        {
            return true;
        }
    }
    false
}

/// 一个词在下载：`Net.WebClient` 的 `DownloadString`、`DownloadData`。
fn downloads(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("downloadstring") || lower.contains("downloaddata")
}

/// 一段原文在下载：下载的命令，或者 [`downloads`]。
fn fetches(text: &str) -> bool {
    downloads(text)
        || lex::parse(text).pipelines.iter().flatten().any(|simple| {
            words(&simple.words)
                .first()
                .is_some_and(|word| fetcher(&base(&word.text)))
        })
}
