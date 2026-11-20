//! 私密路径门禁（施工 SCRUB-1）：仓库是公开的，被 git 跟踪的文本里（`docs/`、`internal/` 除外）不许出现开发机上真实用户的痕迹：
//! `/home/<名字>`、`/tmp/claude-<数字>`、`-home-<名字>-Documents`（后者是按工作目录起的会话存档目录名）。
//!
//! 测试里要用家目录的，写 [`FICTIONAL`] 里的虚构名字。这里不写任何具体的人名、机器名、地址：规则只认形状。

use std::path::Path;
use std::process::Command;

/// 允许出现在 `/home/<名字>` 里的虚构名字（测试夹具、文档样例用的）。名字里 `.`、`-`、`_` 之前的那一段在表里也算
/// （`ai.old`、`ai_2` 这类同一个夹具的变体）。
const FICTIONAL: &[&str] = &[
    "a", "u", "w", "me", "ai", "aim", "alice", "alicex", "bob", "admin", "builder", "someone",
    "user", "example", "test",
];

/// 不扫的目录前缀（相对仓库根）。
const SKIPPED: &[&str] = &["docs/", "internal/"];

/// 名字里能出现的字符。
fn is_name(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')
}

/// 一段文本里每一处违例，说明带行号。
pub fn scan(label: &str, text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let at = index + 1;
        for name in names_after(line, "/home/") {
            if !allowed(name) {
                found.push(format!(
                    "{label}:{at}：`/home/{name}` 像真实用户的家目录，测试里换成虚构的名字（见 xtask/src/privacy.rs）"
                ));
            }
        }
        if line
            .match_indices("/tmp/claude-")
            .any(|(i, m)| line[i + m.len()..].starts_with(|c: char| c.is_ascii_digit()))
        {
            found.push(format!(
                "{label}:{at}：出现了开发机上会话的临时目录 `/tmp/claude-<数字>`，删掉或换成占位"
            ));
        }
        for name in names_after(line, "-home-") {
            if name.ends_with("-Documents") && name.len() > "-Documents".len() {
                found.push(format!(
                    "{label}:{at}：出现了按工作目录起的会话存档目录名 `-home-…-Documents`，删掉或换成占位"
                ));
            }
        }
    }
    found
}

/// `line` 里每个 `marker` 后面紧跟着的一串名字字符。
fn names_after<'a>(line: &'a str, marker: &str) -> Vec<&'a str> {
    line.match_indices(marker)
        .map(|(i, m)| {
            let rest = &line[i + m.len()..];
            let end = rest.find(|c: char| !is_name(c)).unwrap_or(rest.len());
            &rest[..end]
        })
        .collect()
}

/// 名字空的（`/home/` 后面是 `$`、`<`、`{`、`*` 这类占位）、点开头的（`/home/.config`）和虚构名字都放过。
fn allowed(name: &str) -> bool {
    let base = name.split(['.', '-', '_']).next().unwrap_or("");
    name.is_empty()
        || name.starts_with('.')
        || FICTIONAL.contains(&name)
        || FICTIONAL.contains(&base)
}

/// 门禁结果表里的一行。
pub fn outcome(root: &Path) -> crate::Outcome {
    println!("\n── 私密路径：被跟踪的文本里（docs/、internal/ 除外）不许有真实用户的路径 ──");
    crate::Outcome {
        name: "私密路径",
        problems: check(root),
    }
}

/// 门禁这一项：仓库里被 git 跟踪的文本文件。没有 `.git` 的拷贝（远程门禁同步过去的那份）照目录走一遍，跳过
/// `target/`、`.git/`。
pub fn check(root: &Path) -> Vec<String> {
    let names = match tracked(root) {
        Some(names) => names,
        None => {
            let mut names = Vec::new();
            walk(root, Path::new(""), &mut names);
            names
        }
    };
    let mut problems = Vec::new();
    for name in names {
        if SKIPPED.iter().any(|prefix| name.starts_with(prefix)) {
            continue;
        }
        // 二进制、读不了的（已删未提交的、链接）不算文本。
        let Ok(bytes) = std::fs::read(root.join(&name)) else {
            continue;
        };
        if let Ok(text) = String::from_utf8(bytes) {
            problems.extend(scan(&name, &text));
        }
    }
    problems
}

/// `git ls-files` 列的文件；不在 git 仓库里的是 `None`。
fn tracked(root: &Path) -> Option<Vec<String>> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["ls-files", "-z"])
        .output()
        .ok()
        .filter(|out| out.status.success())?;
    Some(
        String::from_utf8_lossy(&out.stdout)
            .split('\0')
            .filter(|name| !name.is_empty())
            .map(String::from)
            .collect(),
    )
}

/// 没有 git 时：每个文件的相对路径，一律 `/`。
fn walk(root: &Path, rest: &Path, names: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(root.join(rest)) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let path = rest.join(&name);
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            if name != "target" && name != ".git" {
                walk(root, &path, names);
            }
        } else if kind.is_file() {
            names.push(path.to_string_lossy().replace('\\', "/"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 违例的样子在测试里拼出来：这份文件自己也被扫，不能整句写在源码里。
    fn home(name: &str) -> String {
        ["/ho", "me/", name].concat()
    }

    #[test]
    fn a_real_looking_home_is_caught_with_its_line() {
        let text = format!("fine\nlet p = \"{}/x\";\n", home("zhangsan"));
        let found = scan("a.rs", &text);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("a.rs:2："), "{found:?}");
    }

    #[test]
    fn fictional_homes_and_placeholders_pass() {
        let text = [
            home("alice"),
            home("ai.old"),
            home("ai_2/x"),
            home("$USER"),
            home("<name>"),
            home(".local"),
            "/ho".to_string() + "me/",
        ]
        .join("\n");
        assert_eq!(scan("a.rs", &text), Vec::<String>::new());
    }

    #[test]
    fn session_temp_dirs_and_archive_names_are_caught() {
        let temp = ["/tmp/", "claude-", "1000/x"].concat();
        let archive = ["-ho", "me-zhangsan-", "Documents"].concat();
        assert_eq!(scan("a.md", &temp).len(), 1);
        assert_eq!(scan("a.md", &archive).len(), 1);
        assert!(scan("a.md", "/tmp/claude-code/x and -home-x").is_empty());
    }

    #[test]
    fn a_tracked_file_with_a_violation_fails_the_check() {
        let dir = std::env::temp_dir().join(format!("xtask-privacy-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("docs")).expect("建得了目录");
        let git = |args: &[&str]| {
            Command::new("git")
                .current_dir(&dir)
                .args(args)
                .output()
                .expect("跑得了 git")
        };
        git(&["init", "-q"]);
        std::fs::write(dir.join("a.txt"), home("zhangsan")).expect("写得进");
        std::fs::write(dir.join("docs/b.txt"), home("zhangsan")).expect("写得进");
        git(&["add", "-A"]);
        let problems = check(&dir);
        // 没有 `.git` 的拷贝照目录走：同样的结果。
        std::fs::remove_dir_all(dir.join(".git")).expect("删得掉");
        let copied = check(&dir);
        std::fs::remove_dir_all(&dir).expect("删得掉");
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].starts_with("a.txt:1："), "{problems:?}");
        assert_eq!(copied, problems);
    }
}
