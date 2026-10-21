//! Windows 上真跑助手（施工 5-9 改上，`docs/blueprint/sandbox/windows.md`）：MXC 的基础容器里，能写的写得进、别处写不了；藏起来
//! 的读不到；别处照设计 11 A4 读得到；退出码、标准输入照原样。没有桌面会话、起不了基础容器的机器上（CI 的 Windows 多半如此）
//! 探测不报手段，助手照实拒绝，说为什么。

#![cfg(windows)]

mod support;

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use miyu_sandbox::{EXIT_HELPER, Spec};
use support::{Dir, HELPER};

/// 这台机器收得住命令：真跑一条试（探测报手段、却没有桌面会话的，跑的时候照实拒绝）。
fn confines_here(spec: &str) -> bool {
    cmd(spec, "echo hi", b"").status.code() != Some(i32::from(EXIT_HELPER))
}

/// 规格：能写 `write`，藏起来 `hidden`。
fn spec(write: &[&PathBuf], hidden: &[&PathBuf]) -> String {
    Spec {
        write: write.iter().map(|path| (*path).clone()).collect(),
        hidden: hidden.iter().map(|path| (*path).clone()).collect(),
        read: None,
        reveal: Vec::new(),
        mount: false,
        sockets: Vec::new(),
    }
    .to_json()
    .expect("写得成")
}

/// 经助手跑 `cmd /c <script>`，标准输入喂 `input`。脚本里的路径不加引号：`cmd` 不认 `\"` 这种转义，临时目录里也没有空格。
fn cmd(spec: &str, script: &str, input: &[u8]) -> Output {
    let mut child = Command::new(HELPER)
        .args(["run", "--spec", spec, "--", "cmd", "/d", "/c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("起得来");
    child
        .stdin
        .take()
        .expect("接了管道")
        .write_all(input)
        .expect("写得进");
    child.wait_with_output().expect("等得了")
}

#[test]
fn the_container_keeps_writes_and_hidden_paths_where_the_spec_says() {
    let work = Dir::new();
    let other = Dir::new();
    let secret = Dir::new();
    other.file("notes.txt", b"shared notes");
    secret.file("key.txt", b"SECRET");
    let (w, s) = (work.path().to_path_buf(), secret.path().to_path_buf());
    let spec = spec(&[&w], &[&s]);
    if !confines_here(&spec) {
        let out = cmd(&spec, "echo hi", b"");
        assert_eq!(out.status.code(), Some(i32::from(EXIT_HELPER)), "{out:?}");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.starts_with("miyu-sandbox: cannot confine: "),
            "{stderr}"
        );
        eprintln!("这台机器收不住命令，只测了拒绝：{stderr}");
        return;
    }
    let inside = work.path().join("a.txt");
    let out = cmd(
        &spec,
        &format!("echo hi> {} && type {}", inside.display(), inside.display()),
        b"",
    );
    assert_eq!(out.status.code(), Some(0), "能写的写得进：{out:?}");
    let outside = other.path().join("b.txt");
    let out = cmd(&spec, &format!("echo hi> {}", outside.display()), b"");
    assert_ne!(out.status.code(), Some(0), "{out:?}");
    assert!(!outside.exists(), "别处写不了");
    let out = cmd(
        &spec,
        &format!("type {}", secret.path().join("key.txt").display()),
        b"",
    );
    assert!(
        !String::from_utf8_lossy(&out.stdout).contains("SECRET"),
        "藏起来的读不到：{out:?}"
    );
    let out = cmd(
        &spec,
        &format!("type {}", other.path().join("notes.txt").display()),
        b"",
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "shared notes",
        "别处读得到"
    );
    let out = cmd(&spec, "exit /b 7", b"");
    assert_eq!(out.status.code(), Some(7), "退出码照原样");
    let out = cmd(&spec, "findstr x", b"axb\r\nnope\r\n");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "axb",
        "标准输入照原样"
    );
}
