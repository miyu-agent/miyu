//! 照拆出来的简单命令认（施工 5-17）：外壳剥掉、删的目标照字面换成绝对路径、git 跳过全局选项。几处都认出来的，交回拒的那一种，
//! 都是问的交回最先认出的。

use super::lex::{Simple, Word, parse};
use super::{Kind, Places};

/// 替换套替换最多看这么多层：再深的照问人。
const DEPTH: usize = 8;

/// 认 `command`：认出来的那一种；没有的是空的。
pub fn assess(command: &str, places: &Places<'_>) -> Option<Kind> {
    let mut found = Vec::new();
    look(command, places, 0, &mut found);
    found
        .iter()
        .find(|kind| kind.refused())
        .or_else(|| found.first())
        .copied()
}

fn look(text: &str, places: &Places<'_>, depth: usize, found: &mut Vec<Kind>) {
    if depth > DEPTH {
        found.push(Kind::DeleteUnresolved);
        return;
    }
    let parsed = parse(text);
    for pipeline in &parsed.pipelines {
        if fetch_into_shell(pipeline) {
            found.push(Kind::PipeToShell);
        }
        for simple in pipeline {
            one(simple, places, depth, found);
        }
    }
    for inner in &parsed.inner {
        if fetches(inner) && pipeline_feeds_shell(&parsed) {
            found.push(Kind::PipeToShell);
        }
        look(inner, places, depth + 1, found);
    }
}

/// 一个简单命令：先看它往哪里写，再剥外壳认命令。
fn one(simple: &Simple, places: &Places<'_>, depth: usize, found: &mut Vec<Kind>) {
    if simple.writes.iter().any(|target| device(&target.text)) {
        found.push(Kind::DiskWrite);
    }
    let words = unwrap(&simple.words);
    let Some((first, rest)) = words.split_first() else {
        return;
    };
    let name = base(&first.text);
    // `sh -c '…'`、`eval '…'`：里面的另认。
    if is_shell(name) {
        if let Some(script) = rest
            .iter()
            .position(|word| word.text == "-c")
            .and_then(|at| rest.get(at + 1))
        {
            look(&script.text, places, depth + 1, found);
        }
        return;
    }
    if name == "eval" {
        let script: Vec<&str> = rest.iter().map(|word| word.text.as_str()).collect();
        look(&script.join(" "), places, depth + 1, found);
        return;
    }
    match name {
        "rm" | "rmdir" | "unlink" | "shred" => deleting(rest, places, recursive(rest), found),
        "find" => finding(rest, places, found),
        "git" => git(rest, found),
        "dd" if rest
            .iter()
            .any(|word| word.text.strip_prefix("of=").is_some_and(device)) =>
        {
            found.push(Kind::DiskWrite);
        }
        "wipefs" | "mke2fs" => found.push(Kind::DiskFormat),
        name if name.starts_with("mkfs") => found.push(Kind::DiskFormat),
        "shutdown" | "reboot" | "halt" | "poweroff" => found.push(Kind::Power),
        "init"
            if rest
                .first()
                .is_some_and(|word| matches!(word.text.as_str(), "0" | "6")) =>
        {
            found.push(Kind::Power)
        }
        "systemctl"
            if rest.iter().any(|word| {
                matches!(
                    word.text.as_str(),
                    "poweroff" | "reboot" | "halt" | "suspend" | "hibernate" | "kexec"
                )
            }) =>
        {
            found.push(Kind::Power);
        }
        _ => {}
    }
}

/// 剥掉外壳：`sudo`、`doas`、`env`、`nice`、`nohup`、`timeout`、`time`、`stdbuf`、`command`、`exec`、`xargs`，连同它们的选项、
/// `env` 的 `名字=值`、`timeout` 的时长。
fn unwrap(words: &[Word]) -> &[Word] {
    let mut at = 0;
    // 开头的 `名字=值`（`FOO=1 rm …`）。
    while words.get(at).is_some_and(|word| assignment(&word.text)) {
        at += 1;
    }
    loop {
        // 外壳的选项要带值、命令却在这里结束的（`sudo -u`），下面一次跳过两个，`at` 会越过结尾（鲁棒性 B-1）。
        let Some(word) = words.get(at) else {
            return words.get(at..).unwrap_or_default();
        };
        let name = base(&word.text);
        if !matches!(
            name,
            "sudo"
                | "doas"
                | "env"
                | "nice"
                | "nohup"
                | "timeout"
                | "time"
                | "stdbuf"
                | "command"
                | "exec"
                | "xargs"
        ) {
            return &words[at..];
        }
        at += 1;
        let takes_value = |flag: &str| match name {
            "sudo" => matches!(
                flag,
                "-u" | "-g" | "-C" | "-D" | "-h" | "-p" | "-r" | "-t" | "-U"
            ),
            "doas" => matches!(flag, "-u" | "-C"),
            "env" => matches!(flag, "-u" | "-C" | "-S"),
            "nice" => matches!(flag, "-n"),
            "timeout" => matches!(flag, "-s" | "-k"),
            "stdbuf" => matches!(flag, "-i" | "-o" | "-e"),
            "xargs" => matches!(flag, "-I" | "-L" | "-n" | "-P" | "-s" | "-d" | "-E" | "-a"),
            _ => false,
        };
        while let Some(word) = words.get(at) {
            if word.text.starts_with('-') {
                at += if takes_value(&word.text) { 2 } else { 1 };
            } else if name == "env" && assignment(&word.text) {
                at += 1;
            } else {
                break;
            }
        }
        if name == "timeout" && words.get(at).is_some() {
            at += 1;
        }
    }
}

fn assignment(text: &str) -> bool {
    text.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty() && name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
    })
}

/// 程序名：路径的最后一段（`/bin/rm` 是 `rm`）。
fn base(text: &str) -> &str {
    text.rsplit('/').next().unwrap_or(text)
}

fn is_shell(name: &str) -> bool {
    matches!(name, "sh" | "bash" | "zsh" | "dash" | "ksh" | "fish")
}

/// 是整个目录往下删的：`-r`、`-R`、`--recursive`，连在一起写的也算（`-rf`、`-fR`）。
fn recursive(rest: &[Word]) -> bool {
    rest.iter().any(|word| {
        word.text == "--recursive"
            || (word.text.starts_with('-')
                && !word.text.starts_with("--")
                && word.text.contains(['r', 'R']))
    })
}

/// `rm` 这类：每个目标照字面换成绝对路径再判。
fn deleting(rest: &[Word], places: &Places<'_>, recursive: bool, found: &mut Vec<Kind>) {
    let mut options = true;
    for word in rest {
        if options && word.text == "--" {
            options = false;
            continue;
        }
        if options && word.text.starts_with('-') && word.text.len() > 1 {
            continue;
        }
        if let Some(kind) = target(word, places, recursive) {
            found.push(kind);
        }
    }
}

/// `find <路径…> … -delete` 或者 `-exec rm`：照删那几个起点算。
fn finding(rest: &[Word], places: &Places<'_>, found: &mut Vec<Kind>) {
    let deletes = rest.iter().any(|word| word.text == "-delete")
        || rest.windows(2).any(|pair| {
            matches!(pair[0].text.as_str(), "-exec" | "-execdir")
                && matches!(base(&pair[1].text), "rm" | "shred")
        });
    if !deletes {
        return;
    }
    for word in rest
        .iter()
        .take_while(|word| !word.text.starts_with(['-', '(', '!']))
    {
        if let Some(kind) = target(word, places, true) {
            found.push(kind);
        }
    }
}

/// 删一个目标：照字面换成绝对路径（`dir/*` 照删 `dir` 算），再照 [`Places`] 判。
fn target(word: &Word, places: &Places<'_>, recursive: bool) -> Option<Kind> {
    let Some(path) = absolute(word, places) else {
        return recursive.then_some(Kind::DeleteUnresolved);
    };
    let path = path
        .strip_suffix("/*")
        .or_else(|| path.strip_suffix("/.*"))
        .unwrap_or(&path)
        .to_string();
    let path = if path.is_empty() {
        "/".to_string()
    } else {
        path
    };
    let inside = |dir: &str, of: &str| {
        of == dir || of.starts_with(&format!("{}/", dir.trim_end_matches('/')))
    };
    if path == "/" {
        return Some(Kind::DeleteRoot);
    }
    if let Some(home) = places.home
        && inside(&path, home)
    {
        return Some(Kind::DeleteHome);
    }
    if !in_workspace(&path, places.data_root)
        && (inside(&path, places.data_root) || inside(places.data_root, &path))
    {
        return Some(Kind::DeleteDataRoot);
    }
    if path.rfind('/') == Some(0) {
        return Some(Kind::DeleteSystem);
    }
    if inside(&path, places.cwd) {
        return Some(Kind::DeleteWorkingDir);
    }
    None
}

/// 落在哪个账号的工作区里面（`<数据根>/home/<账号>/workspace/` 下面，不是它本身：规整过的路径不以 `/` 结尾）。
fn in_workspace(path: &str, data_root: &str) -> bool {
    path.strip_prefix(&format!("{}/home/", data_root.trim_end_matches('/')))
        .and_then(|rest| rest.split_once('/'))
        .and_then(|(_, rest)| rest.strip_prefix("workspace/"))
        .is_some()
}

/// 照字面换成绝对路径，去掉 `.`、`..`、多余的 `/`：`~`、`$HOME`、`${HOME}` 开头的接家目录，相对的接工作目录。别的变量、命令
/// 替换说不清，交回空的。
fn absolute(word: &Word, places: &Places<'_>) -> Option<String> {
    let text = word.text.as_str();
    let (base, rest) = if let Some(rest) = text
        .strip_prefix("${HOME}")
        .or_else(|| text.strip_prefix("$HOME"))
    {
        (places.home?, rest)
    } else if word.unresolved {
        return None;
    } else if text == "~" || text.starts_with("~/") {
        (places.home?, &text[1..])
    } else if text.starts_with('/') {
        ("/", text)
    } else {
        (places.cwd, text)
    };
    if rest.contains('$') {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    for part in base.split('/').chain(rest.split('/')) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    Some(format!("/{}", parts.join("/")))
}

/// 裸设备：`/dev/sd…`、`/dev/nvme…`、`/dev/mmcblk…`、`/dev/disk…`、`/dev/vd…`、`/dev/hd…`、`/dev/xvd…`。
fn device(text: &str) -> bool {
    text.strip_prefix("/dev/").is_some_and(|name| {
        ["sd", "nvme", "mmcblk", "disk", "rdisk", "vd", "hd", "xvd"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
    })
}

/// git：跳过全局选项再认子命令。
fn git(rest: &[Word], found: &mut Vec<Kind>) {
    let mut at = 0;
    while let Some(word) = rest.get(at) {
        match word.text.as_str() {
            "-C" | "-c" | "--git-dir" | "--work-tree" | "--namespace" => at += 2,
            text if text.starts_with('-') => at += 1,
            _ => break,
        }
    }
    let Some(sub) = rest.get(at) else {
        return;
    };
    let args: Vec<&str> = rest[at + 1..]
        .iter()
        .map(|word| word.text.as_str())
        .collect();
    let short = |letter: char| {
        args.iter()
            .any(|arg| arg.starts_with('-') && !arg.starts_with("--") && arg.contains(letter))
    };
    let kind = match sub.text.as_str() {
        "push"
            if short('f')
                || args.iter().any(|arg| {
                    arg.starts_with("--force") || (arg.starts_with('+') && arg.len() > 1)
                }) =>
        {
            Some(Kind::GitForcePush)
        }
        "reset" if args.contains(&"--hard") => Some(Kind::GitResetHard),
        "clean" if short('f') || args.contains(&"--force") => Some(Kind::GitClean),
        "checkout"
            if args.iter().any(|arg| matches!(*arg, "." | ":/"))
                && !args.iter().any(|arg| matches!(*arg, "-b" | "-B")) =>
        {
            Some(Kind::GitDiscard)
        }
        "restore"
            if args.iter().any(|arg| matches!(*arg, "." | ":/")) && !args.contains(&"--staged") =>
        {
            Some(Kind::GitDiscard)
        }
        "stash"
            if args
                .first()
                .is_some_and(|arg| matches!(*arg, "drop" | "clear")) =>
        {
            Some(Kind::GitStashDrop)
        }
        _ => None,
    };
    found.extend(kind);
}

/// 一条管道里先下载、后面接着 shell 或者解释器（读标准输入的那种）：`curl … | sh`。
fn fetch_into_shell(pipeline: &[Simple]) -> bool {
    let mut fetched = false;
    for simple in pipeline {
        let words = unwrap(&simple.words);
        let Some(first) = words.first() else {
            continue;
        };
        let name = base(&first.text);
        if matches!(name, "curl" | "wget") {
            fetched = true;
        } else if fetched
            && interpreter(name)
            && !words[1..].iter().any(|word| !word.text.starts_with('-'))
        {
            return true;
        }
    }
    false
}

/// `bash <(curl …)`、`sh -c "$(curl …)"`：替换里在下载，外面是 shell、解释器。
fn pipeline_feeds_shell(parsed: &super::lex::Parsed) -> bool {
    parsed.pipelines.iter().flatten().any(|simple| {
        unwrap(&simple.words)
            .first()
            .is_some_and(|word| interpreter(base(&word.text)))
    })
}

fn fetches(text: &str) -> bool {
    parse(text).pipelines.iter().flatten().any(|simple| {
        unwrap(&simple.words)
            .first()
            .is_some_and(|word| matches!(base(&word.text), "curl" | "wget"))
    })
}

fn interpreter(name: &str) -> bool {
    is_shell(name) || matches!(name, "python" | "python3" | "perl" | "ruby" | "node")
}
