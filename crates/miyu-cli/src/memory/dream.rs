//! `miyu memory dream`（施工 R-7 补；R-23 加叫停，从 `memory.rs` 挪出来）：发 `memory.dream`，等它回应。等的时候按一次 Ctrl+C
//! 发 `memory.dream_stop`（同一间、同一条连接：`memory.dream` 在后台答，不挡后面的请求），接着等原来那一条回应，印叫停以前合完
//! 了的几样数，退出码 3（同 `miyu ask` 被打断）；再按一次直接退。

use std::io::Write;

use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::refused;
use crate::exit;
use crate::language::Language;
use crate::rpc::Rpc;
use crate::shown::say;

/// 整理 `params` 那一间，照回应印一句；`presses` 是一次次的 Ctrl+C。
///
/// # Errors
///
/// 退出码：被拒绝、核心断开、写不出去的 1，叫停了的、按了两次的 3。
pub(super) async fn dream(
    rpc: &mut Rpc,
    params: Value,
    language: &Language,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mut presses: mpsc::Receiver<()>,
) -> Result<(), u8> {
    let sent = |error: std::io::Error, err: &mut dyn Write| {
        say(err, &error.to_string());
        exit::ERROR
    };
    let id = rpc
        .send("memory.dream", params.clone())
        .await
        .map_err(|error| sent(error, err))?;
    let (mut stopping, mut listening) = (false, true);
    loop {
        tokio::select! {
            message = rpc.next() => {
                let Some(message) = message else {
                    say(err, &language.disconnected());
                    return Err(exit::ERROR);
                };
                // 推送、叫停的那一条的回应（没在整理的：正好合完了）都不用管，照原来那一条说。
                if message["id"] != json!(id) {
                    continue;
                }
                if let Some(error) = message.get("error") {
                    say(err, &refused(error, language));
                    return Err(exit::ERROR);
                }
                return said(&message["result"], language, out);
            }
            press = presses.recv(), if listening => match press {
                None => listening = false,
                Some(()) if stopping => return Err(exit::INTERRUPTED),
                Some(()) => {
                    stopping = true;
                    rpc.send("memory.dream_stop", params.clone())
                        .await
                        .map_err(|error| sent(error, err))?;
                    say(err, language.dream_stopping());
                }
            },
        }
    }
}

/// 整理的回应印一句：叫停了的说中断了、退出码 3。
fn said(result: &Value, language: &Language, out: &mut dyn Write) -> Result<(), u8> {
    let count = |field: &str| result[field].as_u64().unwrap_or(0);
    let (given, revised, retired) = (count("given"), count("revised"), count("retired"));
    if result["stopped"].as_bool().unwrap_or(false) {
        say(out, &language.dream_stopped(given, revised, retired));
        return Err(exit::INTERRUPTED);
    }
    let summary = result["summary"].as_bool().unwrap_or(false);
    say(out, &language.dreamed(given, revised, retired, summary));
    Ok(())
}
