//! 只准读名单里的（施工 5-12 再补，外部身份的会话）：名单外的文件读不到、目录列不出，名单里的照常读、能写的照常写；`/bin/sh`、
//! 系统的命令照样跑得起来（系统目录和程序要的几处在名单里），名单外的文件元数据看得到（同 Linux 的 Landlock）。

use std::path::PathBuf;
use std::process::Output;

use super::*;
use crate::support::{Dir, serial};

/// 会话层给外部身份的会话的名单里，程序要的那几样（`miyu-session` 的 `sandbox/confined.rs`、`miyu-fs` 的系统目录）。
const PROGRAMS: &[&str] = &[
    "/usr",
    "/bin",
    "/sbin",
    "/System",
    "/Library",
    "/opt",
    "/private/etc",
    "/dev",
    "/private/var/select",
    "/private/var/db/timezone",
    "/private/var/db/dyld",
];

/// 测试里经助手跑的几个程序：系统日志里只看它们的。
const OURS: &[&str] = &[
    "sh(",
    "bash(",
    "zsh(",
    "date(",
    "env(",
    "cat(",
    "ls(",
    "test(",
    "miyu-sandbox(",
];

/// 系统日志里沙盒拦下这几个程序的几行（`deny(…)`，`sysctl-read` 不算：整盘能读时也拦，不碍事），和最近一份崩溃报告的
/// 开头：测试红了时附上，CI 上才看得出缺了哪一处。
fn denials() -> String {
    let logged = Command::new("/usr/bin/log")
        .args(["show", "--last", "2m", "--style", "compact", "--predicate"])
        .arg(r#"eventMessage CONTAINS "deny(" AND NOT eventMessage CONTAINS "sysctl-read""#)
        .output()
        .map(|out| {
            let text = String::from_utf8_lossy(&out.stdout).into_owned();
            let ours: Vec<&str> = text
                .lines()
                .filter(|line| {
                    OURS.iter()
                        .any(|name| line.contains(&format!("Sandbox: {name}")))
                })
                .collect();
            ours[ours.len().saturating_sub(60)..].join("\n")
        })
        .unwrap_or_default();
    let reports = std::env::var("HOME")
        .map(|home| PathBuf::from(home).join("Library/Logs/DiagnosticReports"))
        .ok()
        .and_then(|dir| std::fs::read_dir(dir).ok())
        .map(|entries| {
            let mut newest: Vec<(std::time::SystemTime, PathBuf)> = entries
                .filter_map(Result::ok)
                .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
                .collect();
            newest.sort();
            newest
                .last()
                .and_then(|(_, path)| std::fs::read_to_string(path).ok())
                .map(|text| text.lines().take(60).collect::<Vec<_>>().join("\n"))
                .unwrap_or_default()
        })
        .unwrap_or_default();
    format!("{logged}\n--- 最近的崩溃报告 ---\n{reports}")
}

/// 做成了：退出码 0，交回标准输出；没成的附上沙盒拦下的几行。
fn ran(out: &Output) -> String {
    assert_eq!(
        out.status.code(),
        Some(0),
        "{out:?}\n沙盒拦下的：\n{}",
        denials()
    );
    text(&out.stdout)
}

/// 能写 `work`、只准读程序要的和 `work` 的规格。
fn listed(work: &Path) -> String {
    let read: Vec<PathBuf> = PROGRAMS
        .iter()
        .map(PathBuf::from)
        .chain([work.to_path_buf()])
        .collect();
    serde_json::json!({ "write": [work], "hidden": [], "read": read }).to_string()
}

#[test]
fn a_read_list_keeps_everything_else_unreadable() {
    let _serial = serial();
    let work = Dir::new();
    let other = Dir::new();
    let cwd = real(work.path());
    let mine = real(&work.file("mine.txt", b"mine\n"));
    let secret = real(&other.file("secret.txt", b"keep out\n"));
    let spec = listed(&cwd);
    assert_eq!(
        ran(&sh(
            &spec,
            &cwd,
            &format!(
                "cat '{}' && echo new > new.txt && cat new.txt",
                mine.display()
            )
        )),
        "mine\nnew\n"
    );
    // 系统的命令照样跑：找程序、读系统的配置、时区。
    ran(&run(&spec, &cwd, "/usr/bin/env", &["true"]));
    ran(&run(&spec, &cwd, "/bin/date", &[]));
    let home = std::env::var("HOME").expect("有家目录");
    let other_dir = real(other.path());
    for script in [
        format!("cat '{}'", secret.display()),
        format!("ls '{}'", other_dir.display()),
        format!("ls '{home}'"),
    ] {
        denied(&sh(&spec, &cwd, &script));
    }
    // 元数据看得到：知道名字的，在不在、多大照样看得到，内容读不到。
    assert_eq!(
        ran(&sh(
            &spec,
            &cwd,
            &format!("test -f '{}' && echo there", secret.display())
        )),
        "there\n"
    );
}

#[test]
fn hidden_places_stay_hidden_inside_a_read_list() {
    let _serial = serial();
    let work = Dir::new();
    let cwd = real(work.path());
    let data = cwd.join("data");
    std::fs::create_dir(&data).expect("建得了目录");
    std::fs::write(data.join("token"), b"secret\n").expect("写得进");
    let read: Vec<PathBuf> = PROGRAMS
        .iter()
        .map(PathBuf::from)
        .chain([cwd.clone()])
        .collect();
    let spec = serde_json::json!({ "write": [&cwd], "hidden": [&data], "read": read }).to_string();
    denied(&sh(&spec, &cwd, "cat data/token"));
    let looked = sh(&spec, &cwd, "test -e data/token && echo there");
    assert!(!text(&looked.stdout).contains("there"), "{looked:?}");
}
