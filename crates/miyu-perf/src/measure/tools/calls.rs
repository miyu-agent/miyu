//! 工具一步量哪几种调用（施工 V-29）：工具名、参数照出厂预设（`full`）给模型的工具表（`resources/software/basesystem/tools/*.json`），
//! 和要用的工作区。
//!
//! 工作区是一个新的 git 仓库：`grep`、`glob` 在仓库里时照仓库根往下的 `.gitignore` 走（`miyu-basesystem` 的 `walk.rs`），
//! 量尺的沙盒在源码树的 `target/` 下，不另起一个仓库的话整棵树都被源码树的 `.gitignore` 挡掉。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

/// 小文件几行：两 KB 上下。
const SMALL_LINES: usize = 40;

/// 大文件几行：一行 80 个字节，8 MB。
const BIG_LINES: usize = 100_000;

/// 大文件读最后几行：从这么多行以前读起，`read` 要把前面的都数过去。
const BIG_TAIL: usize = 200;

/// 一种调用。
#[derive(Debug, Clone, Copy)]
pub struct Kind {
    /// 表里的名字。
    pub name: &'static str,
    /// 工具名。
    pub tool: &'static str,
    /// 在完全放开下跑（沙盒关着）；不是的在工作区这一级（`shell` 关进沙盒）。
    pub full: bool,
    /// 第几次调用的参数。
    args: fn(&Workspace, usize) -> Value,
}

impl Kind {
    /// 第 `n` 次（从 0 数）调用的参数。
    pub fn args(&self, workspace: &Workspace, n: usize) -> Value {
        (self.args)(workspace, n)
    }
}

/// 量的几种，照这个先后量。
pub const KINDS: [Kind; 14] = [
    Kind {
        name: "read 小文件",
        tool: "read",
        full: false,
        args: read_small,
    },
    Kind {
        name: "read 8 MB 文件的末尾",
        tool: "read",
        full: false,
        args: read_big,
    },
    Kind {
        name: "read 列目录",
        tool: "read",
        full: false,
        args: read_dir,
    },
    Kind {
        name: "write 4 KB",
        tool: "write",
        full: false,
        args: write,
    },
    Kind {
        name: "edit 一处",
        tool: "edit",
        full: false,
        args: edit,
    },
    Kind {
        name: "grep 树里搜",
        tool: "grep",
        full: false,
        args: grep,
    },
    Kind {
        name: "grep 树里搜（带行）",
        tool: "grep",
        full: false,
        args: grep_content,
    },
    Kind {
        name: "glob 树里找",
        tool: "glob",
        full: false,
        args: glob,
    },
    Kind {
        name: "shell true",
        tool: "shell",
        full: false,
        args: shell_true,
    },
    Kind {
        name: "shell echo",
        tool: "shell",
        full: false,
        args: shell_echo,
    },
    Kind {
        name: "shell cat 小文件",
        tool: "shell",
        full: false,
        args: shell_cat,
    },
    Kind {
        name: "shell true（沙盒关着）",
        tool: "shell",
        full: true,
        args: shell_true,
    },
    Kind {
        name: "shell echo（沙盒关着）",
        tool: "shell",
        full: true,
        args: shell_echo,
    },
    Kind {
        name: "shell cat 小文件（沙盒关着）",
        tool: "shell",
        full: true,
        args: shell_cat,
    },
];

/// 工作区里的几样。
#[derive(Debug, Clone)]
pub struct Workspace {
    /// 工作区，会话的工作目录。
    pub dir: PathBuf,
    /// 小文件。
    pub small: PathBuf,
    /// 8 MB 的文件。
    pub big: PathBuf,
    /// 改的文件。
    pub edited: PathBuf,
    /// 写的文件。
    pub written: PathBuf,
    /// 拷进来的树：`grep`、`glob` 在它里面找。
    pub tree: PathBuf,
}

impl Workspace {
    /// 在 `dir` 下建好（有旧的先删掉）：几份文件，`tree` 拷进来（不拷 `target/`）。
    ///
    /// # Errors
    ///
    /// 删不掉、建不了、拷不了。
    pub fn prepare(dir: &Path, tree: &Path) -> Result<Workspace, String> {
        if dir.exists() {
            std::fs::remove_dir_all(dir).map_err(|e| format!("删不掉 {}：{e}", dir.display()))?;
        }
        std::fs::create_dir_all(dir).map_err(|e| format!("建不了 {}：{e}", dir.display()))?;
        let workspace = Workspace {
            dir: dir.to_path_buf(),
            small: dir.join("small.txt"),
            big: dir.join("big.txt"),
            edited: dir.join("edit.txt"),
            written: dir.join("out.txt"),
            tree: dir.join("tree"),
        };
        let write = |path: &Path, text: String| {
            std::fs::write(path, text).map_err(|e| format!("写不进 {}：{e}", path.display()))
        };
        write(&workspace.small, lines(SMALL_LINES, 50))?;
        write(&workspace.big, lines(BIG_LINES, 80))?;
        write(
            &workspace.edited,
            format!("{}value-0\n{}", lines(25, 80), lines(25, 80)),
        )?;
        copy_tree(tree, &workspace.tree)?;
        init_repo(dir)?;
        Ok(workspace)
    }

    /// 绝对路径写成字。
    fn text(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }
}

/// `count` 行，每行 `width` 个字节（连换行），行号打头。
fn lines(count: usize, width: usize) -> String {
    let mut text = String::with_capacity(count * width);
    for n in 1..=count {
        let head = format!("line {n:06} ");
        text.push_str(&head);
        text.push_str(&"x".repeat(width.saturating_sub(head.len() + 1)));
        text.push('\n');
    }
    text
}

/// 拷一棵树：目录、普通文件照拷，`target/` 和链接跳过。
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("建不了 {}：{e}", to.display()))?;
    let entries = std::fs::read_dir(from).map_err(|e| format!("读不了 {}：{e}", from.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读不了 {}：{e}", from.display()))?;
        let kind = entry
            .file_type()
            .map_err(|e| format!("读不了 {}：{e}", entry.path().display()))?;
        let target = to.join(entry.file_name());
        if kind.is_dir() && entry.file_name() != "target" {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), &target)
                .map_err(|e| format!("拷不了 {}：{e}", entry.path().display()))?;
        }
    }
    Ok(())
}

/// 工作区起成一个 git 仓库：有 `git` 的照 `git init`，没有的建一个空的 `.git` 目录（走目录的只看它在不在）。
fn init_repo(dir: &Path) -> Result<(), String> {
    let inited = Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if inited {
        return Ok(());
    }
    std::fs::create_dir_all(dir.join(".git"))
        .map_err(|e| format!("建不了 {}：{e}", dir.join(".git").display()))
}

fn read_small(workspace: &Workspace, _: usize) -> Value {
    json!({"file_path": Workspace::text(&workspace.small)})
}

fn read_big(workspace: &Workspace, _: usize) -> Value {
    json!({"file_path": Workspace::text(&workspace.big), "offset": BIG_LINES - BIG_TAIL + 1,
        "limit": BIG_TAIL})
}

fn read_dir(workspace: &Workspace, _: usize) -> Value {
    json!({"file_path": Workspace::text(&workspace.tree.join("miyu-session").join("src"))})
}

fn write(workspace: &Workspace, n: usize) -> Value {
    json!({"file_path": Workspace::text(&workspace.written),
        "content": format!("written {n}\n{}", lines(50, 80))})
}

fn edit(workspace: &Workspace, n: usize) -> Value {
    json!({"file_path": Workspace::text(&workspace.edited),
        "edits": [{"old_string": format!("value-{n}\n"), "new_string": format!("value-{}\n", n + 1)}]})
}

fn grep(workspace: &Workspace, _: usize) -> Value {
    json!({"pattern": "fn main", "path": Workspace::text(&workspace.tree)})
}

fn grep_content(workspace: &Workspace, _: usize) -> Value {
    json!({"pattern": "fn main", "path": Workspace::text(&workspace.tree), "output_mode": "content"})
}

fn glob(workspace: &Workspace, _: usize) -> Value {
    json!({"pattern": "*.rs", "path": Workspace::text(&workspace.tree)})
}

fn shell(command: String, n: usize) -> Value {
    json!({"command": command, "description": format!("perf step {n}")})
}

fn shell_true(_: &Workspace, n: usize) -> Value {
    shell("true".to_string(), n)
}

fn shell_echo(_: &Workspace, n: usize) -> Value {
    shell("echo perf".to_string(), n)
}

fn shell_cat(workspace: &Workspace, n: usize) -> Value {
    shell(format!("cat '{}'", Workspace::text(&workspace.small)), n)
}
