//! 私密路径门禁（施工 SCRUB-1）：仓库是公开的，被 git 跟踪的文本里不许出现开发机上真实用户的痕迹：
//! `/home/<名字>`、macOS 的 `/Users/<名字>`、Windows 的 `C:\Users\<名字>`（`/`、`\\` 的写法也算）、`/tmp/claude-<数字>`、
//! `-home-<名字>-Documents`（后者是按工作目录起的会话存档目录名）。
//!
//! 测试里要用家目录的，写 [`FICTIONAL`] 里的虚构名字。这里不写任何具体的人名、机器名、地址：规则只认形状。

use std::path::Path;
use std::process::Command;

/// 允许出现在用户目录里的名字（测试夹具、文档样例、系统自带的目录名），不分大小写，整段名字相等才放过：
/// `a-zhangsan` 这类只是开头像的不算。`ai.old`、`ai_2`、`ai-2` 是仓库里同一个夹具的几个写法。
const FICTIONAL: &[&str] = &[
    "a",
    "u",
    "w",
    "me",
    "ai",
    "ai.old",
    "ai_2",
    "ai-2",
    "aim",
    "alice",
    "alicex",
    "bob",
    "admin",
    "builder",
    "someone",
    "user",
    "example",
    "test",
    "runner",
    "runneradmin",
    "public",
    "default",
    "shared",
];

/// 名字里能出现的字符。
fn is_name(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')
}

/// 一段文本里每一处违例，说明带行号。
pub fn scan(label: &str, text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let at = index + 1;
        for (shown, name) in user_dirs(line) {
            if !allowed(&name) {
                found.push(format!(
                    "{label}:{at}：`{shown}{name}` 像真实用户的家目录，测试里换成虚构的名字（见 xtask/src/privacy.rs）"
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
        // `-home-<名字>-Documents-…`：名字取到第一个 `-` 为止，后面的目录名长短不论。
        for name in names_after(line, "-home-") {
            let user = name.split('-').next().unwrap_or("");
            if name[user.len()..].starts_with("-Documents") && !allowed(user) {
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
        .map(|(i, m)| name_at(&line[i + m.len()..]))
        .collect()
}

/// 一行里每个用户目录的（写法，名字）：`/home/`、`/Users/` 和 Windows 的 `X:\Users\`，`\` 和 `\\` 都当 `/`。
/// 前面紧挨着名字字符、`.` 或省略号 `…` 的是路径中间的一段（`pages/home/`、`foo/Users/`、`/api/users/42`），不算；
/// 盘符和 `/mnt/c` 这类 WSL 的挂载点除外。
fn user_dirs(line: &str) -> Vec<(&'static str, String)> {
    // `/home/` 只认正斜杠：`/home\134x` 这类转义（挂载表里的空格写法）不是路径分隔。Windows 的写法才把反斜杠当 `/`。
    let flat = line.replace("\\\\", "/").replace('\\', "/");
    let mut found = Vec::new();
    for (shown, marker, text) in [
        ("/home/", "/home/", line),
        ("/Users/", "/users/", flat.as_str()),
    ] {
        for (i, m) in text.to_ascii_lowercase().match_indices(marker) {
            let before = &text[..i];
            let mount = before.ends_with(':')
                || before
                    .rsplit_once("/mnt/")
                    .is_some_and(|(_, drive)| drive.len() == 1);
            if mount || !before.ends_with(|c| is_name(c) || c == '…') {
                found.push((shown, name_at(&text[i + m.len()..]).to_string()));
            }
        }
    }
    found
}

/// 开头的一串名字字符。
fn name_at(rest: &str) -> &str {
    &rest[..rest.find(|c: char| !is_name(c)).unwrap_or(rest.len())]
}

/// 名字空的（后面是 `$`、`<`、`{`、`*` 这类占位）、点开头的（`/home/.config`）和虚构名字（整段相等）都放过。
/// 句末的 `.`、`-` 不算名字的一部分。
fn allowed(name: &str) -> bool {
    let name = name.trim_end_matches(['.', '-']);
    name.is_empty()
        || name.starts_with('.')
        || FICTIONAL.iter().any(|f| f.eq_ignore_ascii_case(name))
}

/// 门禁结果表里的一行。
pub fn outcome(root: &Path) -> crate::Outcome {
    println!("\n── 私密路径：被跟踪的文本里不许有真实用户的路径 ──");
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
    fn a_name_only_starting_like_a_fictional_one_is_caught() {
        for name in ["a-zhangsan", "u_zhangsan", "alice.zhangsan", "me-zhangsan"] {
            assert_eq!(scan("a.rs", &home(name)).len(), 1, "{name}");
        }
    }

    #[test]
    fn mac_and_windows_user_dirs_are_caught_in_every_spelling() {
        let mac = ["/Us", "ers/zhangsan/x"].concat();
        let win = ["C:", "\\Us", "ers\\zhangsan"].concat();
        let win_slash = ["c:/us", "ers/zhangsan"].concat();
        let win_escaped = ["C:", "\\\\Us", "ers\\\\zhangsan"].concat();
        let wsl = ["/mnt/c/Us", "ers/zhangsan"].concat();
        for text in [mac, win, win_slash, win_escaped, wsl] {
            assert_eq!(scan("a.rs", &text).len(), 1, "{text}");
        }
    }

    #[test]
    fn fictional_mac_and_windows_dirs_and_routes_pass() {
        let text = [
            ["/Us", "ers/me/x"].concat(),
            ["C:", "\\Us", "ers\\ALICE"].concat(),
            ["c:/us", "ers/runneradmin"].concat(),
            ["C:", "\\\\Us", "ers\\\\a"].concat(),
            "GET /api/users/zhangsan and /users/{id}".to_string(),
        ]
        .join("\n");
        assert_eq!(scan("a.rs", &text), Vec::<String>::new());
    }

    #[test]
    fn a_home_or_users_in_the_middle_of_a_path_is_not_a_user_dir() {
        let text = [
            ["pages", "/ho", "me/zhangsan"].concat(),
            ["foo", "/Us", "ers/zhangsan"].concat(),
            ["./Us", "ers/zhangsan"].concat(),
        ]
        .join("\n");
        assert_eq!(scan("a.rs", &text), Vec::<String>::new());
    }

    #[test]
    fn a_backslash_escape_after_home_is_not_a_separator() {
        let text = ["\"/ho", "me\\\\134x\""].concat();
        assert_eq!(scan("a.rs", &text), Vec::<String>::new());
    }

    #[test]
    fn an_elided_prefix_makes_it_a_path_middle() {
        let text = ["cwd=…/ho", "me/home/onebot and …/ho", "me/owner"].concat();
        assert_eq!(scan("a.md", &text), Vec::<String>::new());
    }

    #[test]
    fn documents_are_scanned_too() {
        let dir = std::env::temp_dir().join(format!("xtask-privacy-docs-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("docs")).expect("建得了目录");
        std::fs::create_dir_all(dir.join("internal")).expect("建得了目录");
        for name in ["docs/b.md", "internal/c.md"] {
            std::fs::write(dir.join(name), home("zhangsan")).expect("写得进");
        }
        let problems = check(&dir);
        std::fs::remove_dir_all(&dir).expect("删得掉");
        assert_eq!(problems.len(), 2, "{problems:?}");
    }

    #[test]
    fn a_full_session_archive_name_is_caught_by_its_user_segment() {
        let full = ["-ho", "me-zhangsan-Documents-github-proj"].concat();
        let fictional = ["-ho", "me-alice-Documents-github-proj"].concat();
        assert_eq!(scan("a.md", &full).len(), 1);
        assert!(scan("a.md", &fictional).is_empty());
        assert!(scan("a.md", "-home-zhangsan-notes").is_empty());
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
        std::fs::create_dir_all(&dir).expect("建得了目录");
        let git = |args: &[&str]| {
            Command::new("git")
                .current_dir(&dir)
                .args(args)
                .output()
                .expect("跑得了 git")
        };
        git(&["init", "-q"]);
        std::fs::write(dir.join("a.txt"), home("zhangsan")).expect("写得进");
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
