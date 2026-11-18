//! 挪走、扔进回收站的命令（施工 RO-4，2026-11-18 项目主人定：换个命令删整个数据根也先问）：`mv`、`gio trash`、`gio remove`、
//! `gio move`、`gio rename`、`gvfs-trash`、`trash-put`、`trash`、`kioclient move`、`kioclient remove`（`kioclient5`、`kioclient6`
//! 也算）、带 `--remove-source-files` 的 `rsync`。只管数据根（施工 RO-4补，2026-11-18 项目主人定）：来源是整个数据根或装着它的
//! 照删数据根问（`paths.rs` 的 `moved_away`），别的（家目录、根、系统目录、工作目录也在内）不报，同 RO-4 以前。模型删数据根爱挑
//! 这几种（「进回收站找得回」），原来一个都不认，整个数据根不问就进了回收站（文档会话实测）。
//!
//! 不当整个目录往下删：来源说不清（变量、命令替换）的不报，`mv "$f" …` 太常见。只认常见的这几种，不追全（同模块的说明：防的是
//! 她一时糊涂，不是刻意绕的人）。

use super::lex::Word;

/// `name` 是挪走、扔进回收站的命令时交回来源（`file://` 开头的去掉这一截）；不是的交回空的。`rest` 是程序名后面的词。
pub(super) fn sources(name: &str, rest: &[Word]) -> Option<Vec<Word>> {
    let found = match name {
        // trash-cli、macOS 自带的 `trash`：`--trash-dir` 带值。
        "trash-put" | "trash" | "gvfs-trash" => operands(rest, |flag| flag == "--trash-dir"),
        "gio" => {
            let (sub, args) = rest.split_first()?;
            match sub.text.as_str() {
                "trash" | "remove" => operands(args, |_| false),
                "move" => all_but_last(operands(args, |_| false)),
                "rename" => operands(args, |_| false).into_iter().take(1).collect(),
                _ => return None,
            }
        }
        "kioclient" | "kioclient5" | "kioclient6" => {
            let at = rest.iter().position(|word| !word.text.starts_with('-'))?;
            let args = &rest[at + 1..];
            match rest[at].text.as_str() {
                "move" | "mv" => all_but_last(operands(args, |_| false)),
                "remove" | "rm" => operands(args, |_| false),
                _ => return None,
            }
        }
        "mv" => moved(rest),
        "rsync" if rest.iter().any(|word| word.text == "--remove-source-files") => {
            all_but_last(operands(rest, |flag| RSYNC_VALUED.contains(&flag)))
        }
        _ => return None,
    };
    Some(found.into_iter().map(local).collect())
}

/// `rsync` 带值、值另写一个词的选项（不是来源）。
const RSYNC_VALUED: &[&str] = &[
    "-e",
    "--rsh",
    "-f",
    "--filter",
    "--exclude",
    "--include",
    "--exclude-from",
    "--include-from",
    "--files-from",
    "-T",
    "--temp-dir",
    "--backup-dir",
    "--suffix",
    "--chmod",
    "--log-file",
];

/// `mv`：`-t 目录`、`--target-directory=目录` 写了的，别的都是来源；不然最后一个是去处。`-t`、`-S` 带值（连写在短选项末尾的
/// `-ft 目录` 也是），`--target-directory`、`--suffix` 不写 `=` 的也带。
fn moved(rest: &[Word]) -> Vec<Word> {
    let short = |flag: &str| flag.starts_with('-') && !flag.starts_with("--");
    let target = rest.iter().any(|word| {
        word.text.starts_with("--target-directory")
            || (short(&word.text) && word.text.contains('t'))
    });
    let found = operands(rest, |flag| {
        matches!(flag, "--target-directory" | "--suffix")
            || (short(flag) && flag.ends_with(['t', 'S']))
    });
    match target {
        true => found,
        false => all_but_last(found),
    }
}

/// 不是选项的那几个词：`--` 以后的都算；`takes` 认的选项吃掉后面那一个词。
fn operands(rest: &[Word], takes: impl Fn(&str) -> bool) -> Vec<Word> {
    let mut found = Vec::new();
    let mut words = rest.iter();
    let mut options = true;
    while let Some(word) = words.next() {
        if options && word.text == "--" {
            options = false;
        } else if options && word.text.starts_with('-') && word.text.len() > 1 {
            if takes(&word.text) {
                words.next();
            }
        } else {
            found.push(word.clone());
        }
    }
    found
}

/// 去掉最后一个（去处）。
fn all_but_last(mut found: Vec<Word>) -> Vec<Word> {
    found.pop();
    found
}

/// `file:///x` 照 `/x`（`gio`、`kioclient` 收网址）；别的网址（`trash:/`、`sftp://`）照原样，判不出什么。
fn local(word: Word) -> Word {
    match word.text.strip_prefix("file://") {
        Some(path) if path.starts_with('/') => Word {
            text: path.to_string(),
            ..word
        },
        _ => word,
    }
}
