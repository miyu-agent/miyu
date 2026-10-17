//! 脚本当工具（施工 X-12 上，`docs/blueprint/skills.md`「脚本」第 3 条）：照 `#!` 起；参数从环境变量 `MIYU_ARGS_JSON`、标准输入
//! 都拿得到；在这一轮的工作目录里跑；有一个能写的缓存目录；退出码不是 0 的出错；到时整组杀掉、说一句；起不来的说清楚。
//! Python 的那一条三个平台都跑（Windows 上照 `python` 找）；用 `sh` 的只在 Unix 上。

use serde_json::json;

use miyu_kernel::block::Block;
use miyu_tool::Done;

use crate::support::Site;

fn text(done: &Done) -> String {
    done.blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

#[tokio::test]
async fn a_python_script_gets_its_arguments_both_ways() {
    let site = Site::new();
    site.file(
        "scripts/echo.py",
        b"#!/usr/bin/env python3\n# Description: Echo the arguments.\nimport os, sys\nprint('env=' + os.environ['MIYU_ARGS_JSON'])\nprint('stdin=' + sys.stdin.read())\nprint('cache=' + os.path.basename(os.environ['MIYU_SCRIPT_CACHE_DIR']))\nprint('cwd=' + os.path.basename(os.getcwd()))\n",
    );
    let done = site
        .script("scripts/echo.py", json!({"city": "东京"}))
        .await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(
        text(&done),
        "env={\"city\":\"东京\"}\nstdin={\"city\":\"东京\"}\ncache=echo\ncwd=work\n"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_failing_script_is_an_error_with_its_exit_code() {
    let site = Site::new();
    site.file(
        "scripts/fail.sh",
        b"#!/bin/sh\n# Description: Fail.\necho partial\nexit 3\n",
    );
    let done = site.script("scripts/fail.sh", json!({})).await;
    assert!(done.error);
    assert_eq!(text(&done), "partial\nExit code 3\n");
}

#[cfg(unix)]
#[tokio::test]
async fn a_slow_script_is_stopped_at_its_timeout() {
    let site = Site::new();
    site.file(
        "scripts/slow.sh",
        b"#!/bin/sh\n# Description: Slow.\n# Timeout: 1\necho started\nsleep 30\n",
    );
    let started = std::time::Instant::now();
    let done = site.script("scripts/slow.sh", json!({})).await;
    assert!(done.error);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(20),
        "整组杀掉了"
    );
    assert_eq!(
        text(&done),
        "started\nStopped after 1 s because the script took too long.\n"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_missing_interpreter_says_it_could_not_run() {
    let site = Site::new();
    site.file(
        "scripts/odd.sh",
        b"#!/no/such/interpreter\n# Description: Odd.\n",
    );
    let done = site.script("scripts/odd.sh", json!({})).await;
    assert!(done.error);
    assert!(
        text(&done).starts_with("Could not run odd: "),
        "{}",
        text(&done)
    );
}

/// 脚本拿到的环境变量和 `shell` 的一样（4-8 补）：核心的照传，像密钥的、`MIYU_*` 不给；自己那两个照设。
#[cfg(unix)]
#[tokio::test]
async fn a_script_gets_the_same_environment_as_shell() {
    if !crate::support::in_child(
        "script::a_script_gets_the_same_environment_as_shell",
        crate::shell::PROBES,
    ) {
        return;
    }
    let site = Site::new();
    let body = format!(
        "#!/bin/sh\n# Description: Probe.\n{}\necho \"${{MIYU_ARGS_JSON-unset}}\"\n",
        crate::shell::probe_script()
    );
    site.file("scripts/probe.sh", body.as_bytes());
    let done = site.script("scripts/probe.sh", json!({})).await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(text(&done), "plain|unset|:probe|cat|unset\n{}\n");
}
