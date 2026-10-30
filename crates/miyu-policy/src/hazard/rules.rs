//! 照拆出来的简单命令认（施工 5-17）：外壳剥掉、删的目标照字面换成绝对路径、git 跳过全局选项。几处都认出来的，交回拒的那一种，
//! 都是问的交回最先认出的。

use super::lex::{Simple, Word, parse};
use super::paths::{absolute, target};
use super::vars::{Vars, after_keywords};
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
    let mut vars = Vars::new(places.home);
    // 这时的当前目录（施工 AS-9）：单独成句的 `cd` 照它换，`cd` 到说不清的地方以后是空的。
    let mut cwd = Some(places.cwd.to_string());
    for pipeline in &parsed.pipelines {
        if fetch_into_shell(pipeline) {
            found.push(Kind::PipeToShell);
        }
        for simple in pipeline {
            one(simple, places, (&vars, cwd.as_deref()), depth, found);
        }
        vars.assign(pipeline);
        if let Some(next) = moved(pipeline, places, &vars, cwd.as_deref()) {
            cwd = next;
        }
    }
    for inner in &parsed.inner {
        if fetches(inner) && pipeline_feeds_shell(&parsed) {
            found.push(Kind::PipeToShell);
        }
        look(inner, places, depth + 1, found);
    }
}

/// 单独成句的 `cd`、`pushd`（施工 AS-9）：换到哪（不写的是家目录，`-`、说不清的是空的）；`popd` 照说不清；不是的交回空的。
fn moved(
    pipeline: &[Simple],
    places: &Places<'_>,
    vars: &Vars<'_>,
    cwd: Option<&str>,
) -> Option<Option<String>> {
    let [simple] = pipeline else {
        return None;
    };
    let (first, rest) = after_keywords(&simple.words).split_first()?;
    match first.text.as_str() {
        "cd" | "pushd" => {}
        "popd" => return Some(None),
        _ => return None,
    }
    let dir = rest
        .iter()
        .find(|word| !word.text.starts_with('-') || word.text == "-");
    Some(match dir {
        None => places.home.map(str::to_string),
        Some(word) if word.text == "-" => None,
        Some(word) => absolute(&vars.expand(word), places, cwd),
    })
}

/// 一个简单命令：先看它往哪里写，再剥外壳认命令。`here` 是记下的变量和这时的当前目录。
fn one(
    simple: &Simple,
    places: &Places<'_>,
    here: (&Vars<'_>, Option<&str>),
    depth: usize,
    found: &mut Vec<Kind>,
) {
    if simple.writes.iter().any(|target| device(&target.text)) {
        found.push(Kind::DiskWrite);
    }
    let words = unwrap(&simple.words);
    // 经 `xargs` 喂的（施工 AS-9）：目标从管道里来，字面上一个都没有。
    let fed = simple.words[..simple.words.len() - words.len()]
        .iter()
        .any(|word| base(&word.text) == "xargs");
    let Some((first, rest)) = words.split_first() else {
        return;
    };
    let name = base(&first.text);
    // `sh -c '…'`、`bash -lc '…'`：里面的另认（施工 AS-9：`-c` 和别的选项连写的也算，脚本是它后面第一个不是选项的词）。
    if is_shell(name) {
        if let Some(script) = rest
            .iter()
            .position(|word| short_option(&word.text, 'c'))
            .and_then(|at| {
                rest[at + 1..]
                    .iter()
                    .find(|word| !word.text.starts_with('-'))
            })
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
        // 写到设备上（施工 AS-9）：`shred`、`tee` 的哪个目标是设备，`cp` 的最后一个是设备。
        "shred" | "tee" if rest.iter().any(|word| device(&word.text)) => {
            found.push(Kind::DiskWrite)
        }
        "cp" if rest.last().is_some_and(|word| device(&word.text)) => found.push(Kind::DiskWrite),
        "rm" | "rmdir" | "unlink" | "shred" => {
            let recursive = recursive(rest);
            deleting(rest, places, here, recursive, found);
            if fed && recursive {
                found.push(Kind::DeleteUnresolved);
            }
        }
        "find" => finding(rest, places, here, found),
        "git" => git(rest, found),

        "dd" if rest
            .iter()
            .any(|word| word.text.strip_prefix("of=").is_some_and(device)) =>
        {
            found.push(Kind::DiskWrite);
        }
        "wipefs" | "mke2fs" | "blkdiscard" | "mkswap" => found.push(Kind::DiskFormat),
        "sgdisk"
            if rest
                .iter()
                .any(|word| matches!(word.text.as_str(), "-Z" | "--zap-all")) =>
        {
            found.push(Kind::DiskFormat)
        }
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
        // 容器把宿主交出去（施工 5-17 三补）。
        name => found.extend(super::container::container(name, rest)),
    }
}

/// 剥掉外壳：`sudo`、`doas`、`env`、`nice`、`nohup`、`timeout`、`time`、`stdbuf`、`command`、`exec`、`xargs`，连同它们的选项、
/// `env` 的 `名字=值`、`timeout` 的时长。
fn unwrap(words: &[Word]) -> &[Word] {
    let words = after_keywords(words);
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

/// 短选项（`-` 打头、不是 `--`）里有 `letter`：`-c`、`-lc`、`-ec`。
fn short_option(text: &str, letter: char) -> bool {
    text.starts_with('-') && !text.starts_with("--") && text.contains(letter)
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
fn deleting(
    rest: &[Word],
    places: &Places<'_>,
    (vars, cwd): (&Vars<'_>, Option<&str>),
    recursive: bool,
    found: &mut Vec<Kind>,
) {
    let mut options = true;
    for word in rest {
        if options && word.text == "--" {
            options = false;
            continue;
        }
        if options && word.text.starts_with('-') && word.text.len() > 1 {
            continue;
        }
        if let Some(kind) = target(&vars.expand(word), places, cwd, recursive) {
            found.push(kind);
        }
    }
}

/// `find <路径…> … -delete` 或者 `-exec rm`：照删那几个起点算。
fn finding(
    rest: &[Word],
    places: &Places<'_>,
    (vars, cwd): (&Vars<'_>, Option<&str>),
    found: &mut Vec<Kind>,
) {
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
        if let Some(kind) = target(&vars.expand(word), places, cwd, true) {
            found.push(kind);
        }
    }
}

/// 裸设备：`/dev/sd…`、`/dev/nvme…`、`/dev/mmcblk…`、`/dev/disk…`、`/dev/vd…`、`/dev/hd…`、`/dev/xvd…`；LVM、LUKS 的映射设备
/// `/dev/mapper/…`、`/dev/dm-…`，软 RAID `/dev/md…`，`/dev/loop…`、`/dev/nbd…`（施工 AS-9）。
fn device(text: &str) -> bool {
    text.strip_prefix("/dev/").is_some_and(|name| {
        [
            "sd", "nvme", "mmcblk", "disk", "rdisk", "vd", "hd", "xvd", "mapper/", "dm-", "md",
            "loop", "nbd",
        ]
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
        // 只动暂存区的不丢改动；带了 `--worktree`（`-W`）的连工作区一起丢，有 `--staged` 也算（施工 AS-9）。
        "restore"
            if args.iter().any(|arg| matches!(*arg, "." | ":/"))
                && (!(args.contains(&"--staged") || short('S'))
                    || args.contains(&"--worktree")
                    || short('W')) =>
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

/// `bash <(curl …)`、`sh -c "$(curl …)"`：替换里在下载，外面是 shell、解释器；在这个 shell 里执行的 `source`、`.`、`eval`
/// 也算（施工 AS-9）。
fn pipeline_feeds_shell(parsed: &super::lex::Parsed) -> bool {
    parsed.pipelines.iter().flatten().any(|simple| {
        unwrap(&simple.words).first().is_some_and(|word| {
            let name = base(&word.text);
            interpreter(name) || matches!(name, "source" | "." | "eval")
        })
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
