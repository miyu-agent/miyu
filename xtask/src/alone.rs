//! 单独编得过（施工 IP-1）：外面的仓库只拿核心的几个包用，不会像核心自己的工作区那样把别的包开的特性合进来。
//! 2026-11-27 托盘钉 rc.5 编不过：`miyu-ipc` 用了 `tokio::select!`，自己的依赖却没开 `macros`，工作区里别的包开了，
//! 所以整体编、整体测都看不出来。这里每个包单独 `cargo check` 一次，各算各的特性。

use std::path::Path;

use crate::{Outcome, cargo_step};

/// 外面的仓库直接拿去用的包：托盘只用 `miyu-client`，网页另用 `miyu-ipc`、`miyu-store`，语义模型包用 `miyu-config`、
/// `miyu-recall`。终端界面、im 拿的包多，特性合在一起，不在这里一个个查。
const LENT: &[&str] = &[
    "miyu-client",
    "miyu-ipc",
    "miyu-store",
    "miyu-config",
    "miyu-recall",
];

/// 一个个单独编，没编过的记下是哪个包。
pub(crate) fn check(cargo: &str, root: &Path) -> Outcome {
    let mut problems = Vec::new();
    for package in LENT {
        let step = cargo_step(
            cargo,
            root,
            "单独编",
            &["check", "-p", package, "--quiet"],
            &[],
            true,
        );
        problems.extend(
            step.problems
                .into_iter()
                .map(|problem| format!("{package}：{problem}")),
        );
    }
    Outcome {
        name: "单独编",
        problems,
    }
}
