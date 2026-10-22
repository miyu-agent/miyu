//! 前台命令改了她看过的文件（施工 E-D 补）：结果带一个 `file.read`，哈希是改后的，执行器照它换「看过的版本」；她没看过的、
//! 看过但没改的、动了修改时刻内容没变的不带。

use serde_json::json;

use miyu_kernel::id::ContentHash;
use miyu_tool::{Effect, Seen};

use crate::support::Site;

/// 这台机器上的写法：Unix 上是 bash、zsh 的，Windows 上是 PowerShell 的。
fn script<'a>(unix: &'a str, windows: &'a str) -> &'a str {
    if cfg!(windows) { windows } else { unix }
}

/// 场地里 `work/` 下的几个文件，内容是名字，她看过 `seen` 里那几个。
fn seen_files(site: &Site, all: &[&str], seen: &[&str]) -> Seen {
    for name in all {
        site.file(&format!("work/{name}"), name.as_bytes());
    }
    seen.iter()
        .map(|name| {
            (
                site.real(&format!("work/{name}")),
                ContentHash::of(name.as_bytes()),
            )
        })
        .collect()
}

#[tokio::test]
async fn a_file_she_saw_and_her_command_changed_counts_as_seen_again() {
    let site = Site::new();
    let seen = seen_files(&site, &["a.txt", "b.txt", "c.txt"], &["a.txt", "b.txt"]);
    let command = script(
        "printf x >> a.txt; printf y >> c.txt",
        "Add-Content -NoNewline -Path a.txt -Value x; Add-Content -NoNewline -Path c.txt -Value y",
    );
    let done = site
        .done_seen(
            "work",
            "shell",
            json!({"command": command, "description": "Append"}),
            seen,
        )
        .await;
    assert!(!done.error, "{:?}", done.blocks);
    assert_eq!(
        done.effects,
        [Effect::Read {
            path: site.real("work/a.txt"),
            lines: None,
            hash: ContentHash::of(b"a.txtx"),
        }],
        "只有她看过、命令改了的 a.txt；b.txt 没动，c.txt 她没看过"
    );
}

#[tokio::test]
async fn a_touched_file_with_the_same_content_is_left_alone() {
    let site = Site::new();
    let seen = seen_files(&site, &["a.txt"], &["a.txt"]);
    let command = script(
        "sleep 0.05; touch a.txt",
        "Start-Sleep -Milliseconds 50; (Get-Item a.txt).LastWriteTime = Get-Date",
    );
    let done = site
        .done_seen(
            "work",
            "shell",
            json!({"command": command, "description": "Touch"}),
            seen,
        )
        .await;
    assert!(!done.error, "{:?}", done.blocks);
    assert!(done.effects.is_empty(), "内容没变：{:?}", done.effects);
}

/// 命令起来之前外面就改过、命令没碰的：不是她改的，照旧不认，`edit` 照旧要她重读。
#[tokio::test]
async fn a_file_changed_before_the_command_and_left_alone_is_not_hers() {
    let site = Site::new();
    site.file("work/a.txt", b"changed outside");
    let seen = Seen::from([(site.real("work/a.txt"), ContentHash::of(b"what she read"))]);
    let command = script("echo hi", "Write-Output hi");
    let done = site
        .done_seen(
            "work",
            "shell",
            json!({"command": command, "description": "Say hi"}),
            seen,
        )
        .await;
    assert!(!done.error, "{:?}", done.blocks);
    assert!(done.effects.is_empty(), "命令没碰它：{:?}", done.effects);
}
