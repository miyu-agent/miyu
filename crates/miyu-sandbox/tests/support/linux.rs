//! Linux 上真跑助手的几份测试共用的（`linux.rs`、`linux_mounts.rs`）：规格、经助手跑一段脚本、问助手有什么手段。

use std::path::Path;
use std::process::{Command, Output, Stdio};

use miyu_sandbox::{EXIT_HELPER, Probe, Sandboxed, Spec};

use super::HELPER;

/// 一份规格：`write` 能写，`hidden` 藏起来。
pub fn spec(write: &[&Path], hidden: &[&Path]) -> Spec {
    Spec {
        write: write.iter().map(|path| path.to_path_buf()).collect(),
        hidden: hidden.iter().map(|path| path.to_path_buf()).collect(),
        read: None,
        reveal: Vec::new(),
        mount: false,
        sockets: Vec::new(),
    }
}

/// 经助手跑 `sh -c <script>`。报错照英文说（`LC_ALL=C`），不跟着跑测试的机器的语言。
pub fn run(spec: &Spec, script: &str) -> Output {
    run_with(spec, script, &[])
}

/// 同 [`run`]，助手多带几个环境变量 `env`（例如强制走 seccomp 的 `MIYU_SANDBOX_SECCOMP=force`，施工 MB-1）。和核心一样经
/// [`miyu_sandbox::command`] 起（施工 V-47）：只用 Landlock、有藏起来的规格带上这个测试进程里缓存的规则集，别的和 [`run_fresh`]
/// 一样。
pub fn run_with(spec: &Spec, script: &str, env: &[(&str, &str)]) -> Output {
    run_marked(spec, script, env, false)
}

/// 同 [`run`]，规格标了不缓存（施工 V-52，脚本工具那样）：经 [`miyu_sandbox::command`] 起，助手照旧从头建。
pub fn run_uncached(spec: &Spec, script: &str) -> Output {
    run_marked(spec, script, &[], true)
}

/// 经 [`miyu_sandbox::command`] 跑，`uncached` 是标不标不缓存。
fn run_marked(spec: &Spec, script: &str, env: &[(&str, &str)], uncached: bool) -> Output {
    let sandboxed = Sandboxed {
        helper: HELPER.into(),
        spec: spec.clone(),
        env: Vec::new(),
        uncached,
    };
    let args = ["-c".into(), script.into()];
    miyu_sandbox::command(&sandboxed, Path::new("/bin/sh"), &args)
        .expect("拼得成")
        .env("LC_ALL", "C")
        .envs(env.iter().copied())
        .stdin(Stdio::null())
        .output()
        .expect("起得来")
}

/// 同 [`run`]，助手每次从头建（施工 V-47 以前的样子，参数里没有 `--ruleset`）。
pub fn run_fresh(spec: &Spec, script: &str) -> Output {
    Command::new(HELPER)
        .env("LC_ALL", "C")
        .args([
            "run",
            "--spec",
            &spec.to_json().expect("写得成"),
            "--",
            "/bin/sh",
            "-c",
            script,
        ])
        .stdin(Stdio::null())
        .output()
        .expect("起得来")
}

/// 这台机器的内核有没有能用的 Landlock：问助手自己。
pub fn landlock() -> bool {
    mechanisms().iter().any(|name| name == "landlock")
}

/// 助手探到的手段。
pub fn mechanisms() -> Vec<String> {
    let out = Command::new(HELPER).arg("probe").output().expect("起得来");
    let probe: Probe = serde_json::from_slice(&out.stdout).expect("读得懂");
    probe.mechanisms
}

/// 没有 Landlock 的机器上：拒绝执行，交回 `true`，这个测试到此为止。
pub fn refused_without_landlock(spec: &Spec) -> bool {
    if landlock() {
        return false;
    }
    let out = run(spec, "echo ran");
    assert_eq!(out.status.code(), Some(i32::from(EXIT_HELPER)), "{out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.starts_with("miyu-sandbox: cannot confine: landlock is not available: "),
        "{stderr}"
    );
    assert!(out.stdout.is_empty(), "没跑");
    true
}

pub fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// 跑 `script`，要它失败，报的是被拦（`Permission denied`）。
pub fn denied(spec: &Spec, script: &str) {
    let out = run(spec, script);
    assert_ne!(out.status.code(), Some(0), "{script}: {out:?}");
    assert!(
        text(&out.stderr).contains("Permission denied"),
        "{script}: {out:?}"
    );
}

/// 这台机器的助手报不报 `bwrap`（施工 5-16）。
pub fn bwrap() -> bool {
    let out = Command::new(HELPER).arg("probe").output().expect("起得来");
    let probe: Probe = serde_json::from_slice(&out.stdout).expect("读得懂");
    probe.mechanisms.iter().any(|name| name == "bwrap")
}
