//! 量尺自己当扩展驱动（施工 MD-4）：带参数 [`ARG`] 起来，握手，之后每个 `driver.call` 当场交一段字、回答用量；请求里带着记号
//! [`super::PACE`] 的，在一秒里匀着交 [`PIECES`] 段（量流式的 CPU）。别的不理；标准输入读到头就退出。只用标准库读写，不起运行时。

use std::io::{BufRead, Write};
use std::time::Duration;

use serde_json::{Value, json};

/// 量尺照这个参数起来就当扩展驱动（[`serve`]）。
pub const ARG: &str = "driver-extension";

/// 匀着交的时候一秒交几段：和 `--pieces` 的默认一样。
const PIECES: usize = 300;

/// 当扩展驱动。
pub fn serve() -> std::process::ExitCode {
    let mut out = std::io::stdout().lock();
    let hello = json!({"jsonrpc": "2.0", "id": "perf-driver-0", "method": "hello",
        "params": {"protocol": [1, 1], "head": {"kind": "perf-driver", "version": "0"}}});
    if writeln!(out, "{hello}").and_then(|()| out.flush()).is_err() {
        return std::process::ExitCode::FAILURE;
    }
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else {
            break;
        };
        if !line.contains(r#""method":"driver.call""#) {
            continue;
        }
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if answer(&mut out, &message, line.contains(super::PACE)).is_err() {
            break;
        }
    }
    std::process::ExitCode::SUCCESS
}

/// 答一次：开一块正文，交字（匀着的交 [`PIECES`] 段、隔开一秒的均分），收块，回用量。
fn answer(out: &mut impl Write, message: &Value, paced: bool) -> std::io::Result<()> {
    let call = &message["params"]["call"];
    let mut delta = |deltas: Value| {
        let line = json!({"jsonrpc": "2.0", "method": "driver.delta", "params": {"call": call, "deltas": deltas}});
        writeln!(out, "{line}").and_then(|()| out.flush())
    };
    delta(json!([{"start": {"index": 0, "kind": "text"}}, {"text": {"index": 0, "text": "ok"}}]))?;
    if paced {
        let gap = Duration::from_secs(1) / u32::try_from(PIECES).unwrap_or(1);
        for _ in 1..PIECES {
            std::thread::sleep(gap);
            delta(json!([{"text": {"index": 0, "text": " ok"}}]))?;
        }
    }
    delta(json!([{"end": {"index": 0}}]))?;
    let done = json!({"jsonrpc": "2.0", "id": message["id"],
        "result": {"usage": {"uncached": 1, "cache_read": 0, "cache_write": 0, "output": 1}}});
    writeln!(out, "{done}").and_then(|()| out.flush())
}
