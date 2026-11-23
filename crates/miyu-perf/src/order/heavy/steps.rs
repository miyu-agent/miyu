//! 重场景各段共用的几步（施工 LW-30）：问一句、开会话、照剧本说一句等这一轮结束、等扮扩展的做完、等状态。各段的剧本在
//! `heavy/segments.rs`、`heavy/segments2.rs`。哪一步回了错、等不到，都报错停下，报的是在做什么。

use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use serde_json::{Value, json};

use super::setup::Placed;
use super::web::Web;
use crate::fake::Fake;
use crate::fake::script::{Script, ToolCall};
use crate::rpc::Rpc;
use crate::sandbox::Sandbox;

/// 扩展自己开的会话里说的那一句：假模型照它回工具调用（`heavy/segments.rs` 的 `extensions`）。
pub const EXTENSION_SAYS: &str = "perf-heavy-extension-says: look up alpha.";

/// 平台桥往场所会话里灌的话都带它：假模型照它回一次查找。
pub const BRIDGE_SAYS: &str = "perf-heavy-bridge-says";

/// 等一样状态、一轮结束最多多久：核心在模拟器里。
pub const PATIENCE: Duration = Duration::from_secs(600);

/// 等状态时多久问一次。
const POLL: Duration = Duration::from_millis(200);

/// 一段里要用的：沙盒、假模型、布置好的东西、假服务、家目录。
pub struct Ctx<'a> {
    pub sandbox: &'a Sandbox,
    pub fake: &'a mut Fake,
    pub placed: &'a Placed,
    pub web: &'a Web,
    /// 命令行 `miyu` 的家目录。
    pub home: &'a Path,
    /// 说过几句：记号照它编号，每一句都不一样。
    pub said: usize,
}

/// 问一句，交回 `result`；回了错的报出方法和参数。
///
/// # Errors
///
/// 核心回了错、连接断了、等太久。
pub async fn ask(rpc: &mut Rpc, method: &str, params: Value) -> Result<Value, String> {
    rpc.call(method, params.clone())
        .await
        .map_err(|e| format!("{method} {params}：{e}"))
}

/// 在沙盒的工作目录里开一个会话（`more` 是另加的参数，例如人格），订阅它的事件，交回编号。
///
/// # Errors
///
/// 核心拒了。
pub async fn open(ctx: &Ctx<'_>, rpc: &mut Rpc, more: Value) -> Result<String, String> {
    let mut params = json!({"cwd": ctx.sandbox.work.to_string_lossy()});
    if let (Some(params), Some(more)) = (params.as_object_mut(), more.as_object()) {
        params.extend(more.clone());
    }
    let created = ask(rpc, "session.create", params).await?;
    let session = created["session"]
        .as_str()
        .ok_or("session.create 的回应里没有编号")?
        .to_string();
    ask(
        rpc,
        "subscribe",
        json!({"stream": "events", "session": session}),
    )
    .await?;
    Ok(session)
}

/// 在订阅着的 `session` 里说一句（记号带 `what`），假模型照 `calls`（工具名、参数）一次次回工具调用、回完收尾；等这一轮结束。
///
/// # Errors
///
/// 核心拒了、这一轮等不到结束。
pub async fn talk(
    ctx: &mut Ctx<'_>,
    rpc: &mut Rpc,
    session: &str,
    what: &str,
    calls: &[(&str, Value)],
) -> Result<(), String> {
    ctx.said += 1;
    let marker = format!("perf-heavy-{what}-{}", ctx.said);
    ctx.fake.script(Some(script(&marker, calls)));
    let sent = ask(
        rpc,
        "session.send",
        json!({"session": session, "text": format!("{marker}: go on.")}),
    )
    .await;
    let ended = match sent {
        Ok(sent) => turn_after(rpc, session, sent["events"][0].as_u64().unwrap_or(0)).await,
        Err(e) => Err(e),
    };
    ctx.fake.script(None);
    ended.map_err(|e| format!("「{marker}」那一轮：{e}"))
}

/// 照 `calls` 写一份剧本：编号带着记号，整个剧本里不重复。
pub fn script(marker: &str, calls: &[(&str, Value)]) -> Script {
    Script {
        marker: marker.to_string(),
        calls: calls
            .iter()
            .enumerate()
            .map(|(n, (name, args))| ToolCall {
                id: format!("call_{}_{n}", marker.replace('-', "_")),
                name: (*name).to_string(),
                arguments: args.to_string(),
            })
            .collect(),
    }
}

/// 等 `session` 推来序号比 `seq` 大的 `turn.ended`：前面几轮攒着的不算。
///
/// # Errors
///
/// 读不到、等太久。
pub async fn turn_after(rpc: &mut Rpc, session: &str, seq: u64) -> Result<(), String> {
    let ended = |push: &Value| {
        push["method"] == "event"
            && push["params"]["session"] == session
            && push["params"]["event"]["kind"] == "turn.ended"
            && push["params"]["event"]["seq"]
                .as_u64()
                .is_some_and(|s| s > seq)
    };
    rpc.push("这一轮结束", ended).await.map(|_| ())
}

/// 隔 [`POLL`] 问一次 `method`，直到回应合 `wanted`，最多 [`PATIENCE`]；交回合上的那一次回应。
///
/// # Errors
///
/// 回了错、等太久（报最后一次的回应）。
pub async fn until(
    rpc: &mut Rpc,
    what: &str,
    method: &str,
    params: Value,
    wanted: impl Fn(&Value) -> bool,
) -> Result<Value, String> {
    let began = Instant::now();
    loop {
        let got = ask(rpc, method, params.clone()).await?;
        if wanted(&got) {
            return Ok(got);
        }
        if began.elapsed() > PATIENCE {
            return Err(format!("等{what}等了 {} 秒：{got}", PATIENCE.as_secs()));
        }
        tokio::time::sleep(POLL).await;
    }
}

/// `extension.status` 里 `package` 的状态是 `state`。
pub fn extension_is(status: &Value, package: &str, state: &str) -> bool {
    status["extensions"].as_array().is_some_and(|all| {
        all.iter()
            .any(|one| one["package"] == package && one["state"] == state)
    })
}

/// `mcp.list` 里 `names` 这几个服务都在跑。
pub fn servers_running(list: &Value, names: &[&str]) -> bool {
    names.iter().all(|name| {
        list["servers"].as_array().is_some_and(|all| {
            all.iter()
                .any(|one| one["name"] == *name && one["state"] == "running")
        })
    })
}

/// 往控制目录里放一个文件 `name`：停在 `pause` 那一步的扮扩展的接着往下走。
///
/// # Errors
///
/// 写不进。
pub fn release(placed: &Placed, name: &str) -> Result<(), String> {
    let path = placed.ctl.join(name);
    std::fs::write(&path, b"").map_err(|e| format!("写不进 {}：{e}", path.display()))
}

/// 等扮扩展的那一个的记录 `record` 里有 `times` 行 `done`，再查它每一步：回了错的、等不到一轮结束的报出来。
///
/// # Errors
///
/// 等太久、记录读不了、哪一步没成。
pub async fn finished(record: &Path, times: usize) -> Result<(), String> {
    let began = Instant::now();
    loop {
        let text = std::fs::read_to_string(record).unwrap_or_default();
        let lines: Vec<Value> = text
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        if lines.iter().filter(|line| line["kind"] == "done").count() >= times {
            let failed: Vec<String> = lines
                .iter()
                .filter(|line| {
                    (line["kind"] == "call" && !line["body"]["error"].is_null())
                        || (line["kind"] == "turn" && line["body"]["ended"] != true)
                        || (line["kind"] == "hello" && line["body"]["ok"] != true)
                })
                .map(Value::to_string)
                .collect();
            return match failed.is_empty() {
                true => Ok(()),
                false => Err(format!(
                    "{} 里有没做成的：{}",
                    record.display(),
                    failed.join("；")
                )),
            };
        }
        if began.elapsed() > PATIENCE {
            return Err(format!(
                "等了 {} 秒，{} 还没做完：{text}",
                PATIENCE.as_secs(),
                record.display()
            ));
        }
        tokio::time::sleep(POLL).await;
    }
}

/// 照 `words` 跑一次命令行 `miyu`（连上正在运行的核心）。
///
/// # Errors
///
/// 拉不起来、退出码不是 0。
pub fn cli(ctx: &Ctx<'_>, words: &[&str]) -> Result<(), String> {
    ctx.sandbox.cli(words, ctx.home).map(|_| ())
}

/// 从现在起 `seconds` 秒以后的 UTC 时刻，写成 `2026-11-23T04:30:00.000Z`（定时任务的 `at`）。
///
/// # Errors
///
/// 时钟在 1970 年以前。
pub fn utc_after(seconds: u64) -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| format!("时钟在 1970 年以前：{e}"))?
        .as_secs()
        + seconds;
    Ok(utc(now))
}

/// 从 1970 年起的秒数写成 UTC 的时刻（照公历换算：Howard Hinnant 的 `civil_from_days`）。
pub fn utc(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let rest = seconds % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.000Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_is_written_the_way_events_write_it() {
        assert_eq!(utc(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(utc(1_795_400_000), "2026-11-23T02:13:20.000Z");
        assert_eq!(utc(951_782_400), "2000-02-29T00:00:00.000Z");
    }

    #[test]
    fn scripts_carry_unique_ids_and_the_marker() {
        let s = script(
            "perf-heavy-web-3",
            &[
                ("web_fetch", json!({"url": "u"})),
                ("web_search", json!({"query": "q"})),
            ],
        );
        assert_eq!(s.marker, "perf-heavy-web-3");
        assert_eq!(s.calls[1].id, "call_perf_heavy_web_3_1");
        assert_eq!(s.calls[0].arguments, r#"{"url":"u"}"#);
    }
}
