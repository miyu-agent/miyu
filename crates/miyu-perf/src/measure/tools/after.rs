//! 挂着一个慢的 `tool.after` 收件人的工具一步（施工 HK-2，设计 `65-工具挂接点.md` 第六节「量尺」）：收件人慢，不拖回合，
//! 一步工具调用的时长和没有收件人时一样。
//!
//! 收件人是量尺自己：核心起来以前往沙盒的数据根装一个扩展包 `perf-after`（开关关着，`[after] tools = ["read"]`），带参数
//! [`ARG`] 起来只握手，之后每收一行先睡 [`SLOW`] 再读下一行（[`serve`]）：通知越攒越多，核心那边的队列、写队列照样只入队。
//! 别的几种量完打开它，再量一遍「read 小文件」（[`super::calls::TOLD`]），和头一种比；量完关掉，再量守卫那一种。

use std::io::{BufRead, Write};
use std::time::Duration;

use serde_json::json;

use miyu_store::root::DataRoot;

use crate::rpc::Rpc;

/// 收件人的包编号。
const PACKAGE: &str = "perf-after";

/// 量尺照这个参数起来就当慢的收件人（[`serve`]）。
pub const ARG: &str = "after-extension";

/// 每收一行睡多久。
const SLOW: Duration = Duration::from_millis(200);

/// 往数据根 `root` 装上收件人。
///
/// # Errors
///
/// 写不进、找不到、拷不了量尺自己。
pub fn install(root: &DataRoot) -> Result<(), String> {
    super::guard::install_as(
        root,
        (PACKAGE, ARG),
        "events.read",
        "[after]\ntools = [\"read\"]\n",
    )
}

/// 打开收件人、批 `events.read`，等它在跑。
///
/// # Errors
///
/// 核心拒了、等太久还没在跑。
pub async fn enable(rpc: &mut Rpc) -> Result<(), String> {
    super::guard::enable_as(rpc, PACKAGE, "events.read").await
}

/// 关掉收件人：后面量守卫的那一种不带它。
///
/// # Errors
///
/// 核心拒了。
pub async fn disable(rpc: &mut Rpc) -> Result<(), String> {
    rpc.call("extension.disable", json!({"package": PACKAGE}))
        .await
        .map(|_| ())
}

/// 当慢的收件人：握手，之后每读一行睡一会儿；标准输入读到头就退出。
pub fn serve() -> std::process::ExitCode {
    let mut out = std::io::stdout().lock();
    let hello = json!({"jsonrpc": "2.0", "id": "perf-after-0", "method": "hello",
        "params": {"protocol": [1, 1], "head": {"kind": "perf-after", "version": "0"}}});
    if writeln!(out, "{hello}").and_then(|()| out.flush()).is_err() {
        return std::process::ExitCode::FAILURE;
    }
    for line in std::io::stdin().lock().lines() {
        if line.is_err() {
            break;
        }
        std::thread::sleep(SLOW);
    }
    std::process::ExitCode::SUCCESS
}
