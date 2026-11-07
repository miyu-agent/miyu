//! 挂着守卫的工具一步（施工 HK-1，设计 `65-工具挂接点.md` 第六节「量尺」）：挂了一个立即答 `allow` 的守卫时，一步工具调用
//! 多多少（目标 1 毫秒以内）。
//!
//! 守卫是量尺自己：核心起来以前往沙盒的数据根装一个扩展包 `perf-guard`（`start = "manual"`，开关关着，前面那几种量的时候
//! 不问它），程序是拷过去的量尺本身，带参数 [`ARG`] 起来就只握手、每个 `tool.guard` 当场答 `allow`（[`serve`]）。别的几种量完
//! 以后打开它（`extension.enable` 带 `approve`），等它在跑，再量一遍「read 小文件」（[`super::calls::GUARDED`]），和前面那一行一比
//! 就是多出来的：查表、读开关、发出去、等它答、合。

use std::io::{BufRead, Write};
use std::time::Duration;

use serde_json::{Value, json};

use miyu_store::root::DataRoot;

use crate::rpc::{PATIENCE, Rpc};

/// 守卫的包编号。
const PACKAGE: &str = "perf-guard";

/// 量尺照这个参数起来就当守卫（[`serve`]）。
pub const ARG: &str = "guard-extension";

/// 往数据根 `root` 装上守卫：清单、拷过去的量尺本身。
///
/// # Errors
///
/// 写不进、找不到、拷不了量尺自己。
pub fn install(root: &DataRoot) -> Result<(), String> {
    install_as(
        root,
        (PACKAGE, ARG),
        "tool.guard",
        "[guard]\ntools = [\"read\"]\n",
    )
}

/// 往数据根 `root` 装一个量尺自己当程序的扩展包：编号、参数是 `package`、`arg`，声明 `capability`，清单末尾接上 `table`
/// （施工 HK-2 起 `tool.after` 的收件人也照它装）。开关关着（`start = "manual"`）。
///
/// # Errors
///
/// 写不进、找不到、拷不了量尺自己。
pub fn install_as(
    root: &DataRoot,
    (package, arg): (&str, &str),
    capability: &str,
    table: &str,
) -> Result<(), String> {
    let dir = root.packages().join(package);
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).map_err(|e| format!("建不了 {}：{e}", bin.display()))?;
    let manifest = format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Perf {package}\" }}\n\n[command]\nname = \"{package}\"\nprogram = \"{package}\"\nabout = {{ en = \"Perf helper\" }}\n\n[process]\nargs = [\"{arg}\"]\nstart = \"manual\"\ncapabilities = [\"{capability}\"]\n\n{table}"
    );
    std::fs::write(dir.join("package.toml"), manifest).map_err(|e| format!("清单写不进：{e}"))?;
    let me = std::env::current_exe().map_err(|e| format!("找不到量尺自己：{e}"))?;
    let program = bin.join(format!("{package}{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(&me, &program).map_err(|e| format!("拷不了量尺自己：{e}"))?;
    Ok(())
}

/// 打开守卫、批它的能力，等它在跑。
///
/// # Errors
///
/// 核心拒了、等太久还没在跑。
pub async fn enable(rpc: &mut Rpc) -> Result<(), String> {
    enable_as(rpc, PACKAGE, "tool.guard").await
}

/// 打开扩展包 `package`、批它的能力 `capability`，等它在跑（施工 HK-2 起 `tool.after` 的收件人也照它开）。
///
/// # Errors
///
/// 核心拒了、等太久还没在跑。
pub async fn enable_as(rpc: &mut Rpc, package: &str, capability: &str) -> Result<(), String> {
    rpc.call(
        "extension.enable",
        json!({"package": package, "approve": [capability]}),
    )
    .await?;
    let deadline = tokio::time::Instant::now() + PATIENCE;
    loop {
        let status = rpc.call("extension.status", json!({})).await?;
        let running = status["extensions"].as_array().is_some_and(|all| {
            all.iter()
                .any(|one| one["package"] == json!(package) && one["state"] == json!("running"))
        });
        if running {
            return Ok(());
        }
        if tokio::time::Instant::now() > deadline {
            return Err(format!("{package} 一直没在跑：{status}"));
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 当守卫：握手，之后每个 `tool.guard` 当场答 `allow`，别的不理；标准输入读到头就退出。
pub fn serve() -> std::process::ExitCode {
    let mut out = std::io::stdout().lock();
    let hello = json!({"jsonrpc": "2.0", "id": "perf-guard-0", "method": "hello",
        "params": {"protocol": [1, 1], "head": {"kind": "perf-guard", "version": "0"}}});
    if writeln!(out, "{hello}").and_then(|()| out.flush()).is_err() {
        return std::process::ExitCode::FAILURE;
    }
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else {
            break;
        };
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if message["method"] != json!("tool.guard") {
            continue;
        }
        let answer = json!({"jsonrpc": "2.0", "id": message["id"], "result": {"verdict": "allow"}});
        if writeln!(out, "{answer}")
            .and_then(|()| out.flush())
            .is_err()
        {
            break;
        }
    }
    std::process::ExitCode::SUCCESS
}
