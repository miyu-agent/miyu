//! 脚本工具的沙盒规格不进核心的规则集缓存（施工 V-52）：交图目录每次调用一个新的随机名、放进规格的 `write`，规格次次不一样；
//! 脚本工具把它标成不缓存（`Sandboxed::uncached`），照施工 V-47 以前让助手从头建，也不把 `shell` 那份挤出 8 份的缓存。
//!
//! 假助手是 `/bin/echo`：它印出收到的参数，就是调用的输出，看得见有没有 `--ruleset <号码>`。认不认 `shell` 那份是缓存里原来
//! 那一个：测试进程自己拿着一条直接经 `miyu_sandbox::command` 拼的命令（里面拿着那个规则集），那份要是被挤掉重建了，新的
//! 号码一定和拿着的不一样。藏起来的那一条摆在 cargo 的输出目录下：一级级走要列每一级上级，`/tmp` 里同一个测试程序别的测试
//! 一直在建删，`shell` 那份会次次重建（路上别处有人改的那一次再来，5 次里有一次对上就算）。
//!
//! 只在 Linux 上编；这台机器的 Landlock 用不了的（本来就不缓存）不跑。

#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::json;

use miyu_kernel::block::Block;
use miyu_sandbox::{Sandboxed, Spec};
use miyu_tool::{Progress, Seen, Stop};

use crate::support::Site;

/// 接连调几次脚本：比缓存的份数（8）多。
const SCRIPTS: usize = 10;

/// 交回的字。
fn text(done: &miyu_tool::Done) -> String {
    done.blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

/// 参数里 `--ruleset` 后面那个号码；没有的交回没有。
fn ruleset_of<'a>(args: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let mut args = args.into_iter();
    args.by_ref().find(|arg| *arg == "--ruleset")?;
    args.next().map(str::to_owned)
}

/// cargo 的输出目录下一个用完就删的目录，里面藏 `data`，能写 `work`。
struct Tree(PathBuf);

impl Tree {
    fn new() -> Tree {
        let exe = std::env::current_exe().expect("知道测试程序在哪");
        let dir = exe
            .parent()
            .and_then(Path::parent)
            .expect("测试程序在 target/<profile>/deps/ 里")
            .join("miyu-basesystem-v52")
            .join(std::process::id().to_string());
        drop(std::fs::remove_dir_all(&dir));
        for sub in ["data", "work"] {
            std::fs::create_dir_all(dir.join(sub)).expect("建得了");
        }
        Tree(std::fs::canonicalize(dir).expect("在"))
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        drop(std::fs::remove_dir_all(&self.0));
    }
}

#[tokio::test]
async fn script_calls_stay_out_of_the_cache_and_leave_the_shell_entry_in_it() {
    if miyu_sandbox::linux::available().is_err() {
        eprintln!("这台机器的 Landlock 用不了，不缓存，不比");
        return;
    }
    let site = Site::new();
    let tree = Tree::new();
    let spec = Spec::from_json(
        &json!({ "write": [tree.0.join("work")], "hidden": [tree.0.join("data")] }).to_string(),
    )
    .expect("读得懂");
    let sandboxed = Sandboxed {
        helper: "/bin/echo".into(),
        spec,
        env: Vec::new(),
        uncached: false,
    };
    site.file(
        "scripts/nothing.sh",
        b"#!/bin/sh\n# Description: Nothing.\n",
    );
    let script = site.script_tool("scripts/nothing.sh");
    let shell = || async {
        let done = site
            .done_sandboxed(
                "shell",
                json!({ "command": "true", "description": "Test" }),
                sandboxed.clone(),
            )
            .await;
        let printed = text(&done);
        assert!(!done.error, "{printed}");
        ruleset_of(printed.split_whitespace())
            .unwrap_or_else(|| panic!("shell 带缓存的规则集：{printed}"))
    };

    let mut last = String::new();
    for _ in 0..5 {
        // `shell` 那份进缓存；拿着一条同一份规格直接拼的命令，号码和 `shell` 交给助手的一样，就是缓存里那一个。
        let before = shell().await;
        let held = miyu_sandbox::command(&sandboxed, Path::new("/bin/true"), &[]).expect("拼得成");
        let number = ruleset_of(held.get_args().filter_map(|arg| arg.to_str()));
        if number.as_deref() != Some(before.as_str()) {
            last = format!("shell 交的 {before}，直接拼的 {number:?}");
            continue;
        }
        for _ in 0..SCRIPTS {
            let call = miyu_tool::Call {
                sandbox: Some(Arc::new(sandboxed.clone())),
                ..site.call_for("work", json!({}), Seen::new(), Stop::default())
            };
            let done = script.run(call, Progress::new(|_| {})).await;
            let printed = text(&done);
            assert!(!done.error, "{printed}");
            assert!(printed.starts_with("run --spec "), "{printed}");
            assert!(
                ruleset_of(printed.split_whitespace()).is_none(),
                "脚本不带缓存的规则集，助手从头建：{printed}"
            );
        }
        let after = shell().await;
        drop(held);
        if after == before {
            return;
        }
        last = format!("头一次 {before}，调了 {SCRIPTS} 次脚本以后 {after}");
    }
    panic!("shell 那份被挤掉、重建了：{last}");
}
