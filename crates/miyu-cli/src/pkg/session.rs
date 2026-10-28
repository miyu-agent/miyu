//! 会话里的 `miyu pkg`（施工 BS-1 中下，设计 `44-基础系统拆包.md` 第三节第 4 条）：看见 `MIYU_CALL_SOCK`（这条命令跑在 Miyu
//! 的会话里），一律经 `miyu call` 的通道带着这条命令的令牌发 `op: "pkg"`，不读本机令牌、不连核心的协议（同 MC11：退回去就是
//! 借本人的身份，绕过这个会话的权限）。子命令换成调 `packages` 包的两件，印核心答回来的、照它退出。
//!
//! - `list`、`info`、`search`、`-Q`、`-Qi`、`-Ss` 调 `packages`；`info` 写的是路径（带 `/`、`\`）的看那个包文件夹。
//! - `install`、`remove`、`upgrade`、`-S <编号>`、`-Su` 调 `package`；`install` 写的是路径的照现在的目录换成绝对的，写了
//!   `package.toml` 的取它所在的文件夹。
//! - `pack` 照旧在本机打（不连核心，[`super::run`] 先做了）。
//! - 别的（`files`、`owns`、`check`、`update`、`clean`、只有 `-y`、`-c` 的 `-S`）说这里用不了，退出码 2。
//! - `--yes` 不认：问不问由权限策略判，她跳不过。
//! - 通道连不上的（端点不在了、令牌不对）说这里用不了，退出码 2，不退回去走协议。

use std::path::{Component, Path};

use serde_json::{Value, json};

use super::{Pkg, PkgCommand};
use crate::call::{VERSION, exchange, print_out};

/// 用法不对、这里用不了。
const USAGE: u8 = 2;

/// 这个会话没有 `packages` 包、通道连不上。
const OFF: &str = include_str!("../../../../resources/software/basesystem/shell/call/pkg-off.txt");

/// 这个子命令在会话里用不了：`{command}` 换成子命令。
const UNAVAILABLE: &str =
    include_str!("../../../../resources/software/basesystem/shell/call/pkg-unavailable.txt");

/// 读的那件、改的那件。
const PACKAGES: &str = "packages";
const PACKAGE: &str = "package";

/// 在会话里做一次 `args`：相对的路径照 `cwd` 算。交回退出码：一次一次调，有一次不成的照它退出、后面的不做。
pub(super) fn run(args: &Pkg, cwd: &Path) -> u8 {
    let calls = match calls(args.command.clone(), cwd) {
        Ok(calls) => calls,
        Err(command) => {
            print_out("", &UNAVAILABLE.replace("{command}", &command));
            return USAGE;
        }
    };
    for (tool, args) in calls {
        let request = json!({"v": VERSION, "op": "pkg", "tool": tool, "args": args.to_string()});
        let code = match exchange(&request) {
            Ok(reply) => {
                let text = |key: &str| {
                    reply
                        .get(key)
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string()
                };
                print_out(&text("stdout"), &text("stderr"));
                reply
                    .get("code")
                    .and_then(Value::as_u64)
                    .and_then(|code| u8::try_from(code).ok())
                    .unwrap_or(1)
            }
            Err(_) => {
                print_out("", OFF);
                USAGE
            }
        };
        if code != 0 {
            return code;
        }
    }
    0
}

/// 子命令换成的几次调用（工具、参数）；会话里用不了的交回写给她看的子命令。
pub(super) fn calls(
    command: Option<PkgCommand>,
    cwd: &Path,
) -> Result<Vec<(&'static str, Value)>, String> {
    let command = command.unwrap_or(PkgCommand::List).plain();
    let one = |tool, args| Ok(vec![(tool, args)]);
    match command {
        PkgCommand::List => one(PACKAGES, json!({})),
        PkgCommand::Info { package } if is_path(&package) => {
            one(PACKAGES, json!({"path": folder(&package, cwd)}))
        }
        PkgCommand::Info { package } => one(PACKAGES, json!({"package": package})),
        PkgCommand::Search { words } => one(PACKAGES, json!({"search": words.join(" ")})),
        PkgCommand::Install { what } if is_path(&what) => {
            one(PACKAGE, json!({"install": folder(&what, cwd)}))
        }
        PkgCommand::Install { what } => one(PACKAGE, json!({"install": what})),
        PkgCommand::Remove { package } => one(PACKAGE, json!({"remove": package})),
        PkgCommand::Upgrade => one(PACKAGE, json!({"upgrade": "all"})),
        PkgCommand::Sync(flags) if flags.upgrade() => one(PACKAGE, json!({"upgrade": "all"})),
        PkgCommand::Sync(flags) if flags.cleans() => Err("-Sc".to_string()),
        PkgCommand::Sync(flags) => match (flags.installs(), flags.searched()) {
            (Some(ids), _) => Ok(ids
                .iter()
                .map(|id| (PACKAGE, json!({"install": id})))
                .collect()),
            (None, Some(words)) => one(PACKAGES, json!({"search": words})),
            (None, None) => Err("-Sy".to_string()),
        },
        PkgCommand::Files { .. } => Err("files".to_string()),
        PkgCommand::Owns { .. } => Err("owns".to_string()),
        PkgCommand::Check { .. } => Err("check".to_string()),
        PkgCommand::Update => Err("update".to_string()),
        PkgCommand::Clean => Err("clean".to_string()),
        // `pack` 在连核心之前就做了；`-Q` 已经换成了正式的。
        PkgCommand::Pack { .. } | PkgCommand::Query(_) => Err("pack".to_string()),
    }
}

/// 写的是路径（同 `install` 认路径的写法）：带 `/`、`\`，是 `.`、`..`，或者以 `.toml`、`.miyu` 结尾。
fn is_path(what: &str) -> bool {
    what.contains(['/', '\\'])
        || matches!(what, "." | "..")
        || what.ends_with(".toml")
        || what.ends_with(".miyu")
}

/// 绝对路径：相对的照 `cwd` 算，`.`、`..` 照字面去掉；写到 `package.toml` 的取它所在的文件夹，包文件照原样。
fn folder(what: &str, cwd: &Path) -> String {
    let mut path = std::path::PathBuf::new();
    for part in cwd.join(what).components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                path.pop();
            }
            other => path.push(other),
        }
    }
    if path.file_name().is_some_and(|name| name == "package.toml") {
        path.pop();
    }
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests;
