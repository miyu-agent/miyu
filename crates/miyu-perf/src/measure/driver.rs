//! 扩展提供的模型驱动比内置驱动多多少（施工 MD-4，设计 `66-扩展提供模型驱动.md` 第六节，`23-性能预算.md` 第二节那两行，core1
//! 11-07 照推荐定：进预算、CI 的 perf 那一项拦）。
//!
//! 扩展是量尺自己：核心起来以前往沙盒的数据根装一个扩展包 `perf-driver`（照工具一步的守卫那样，`tools/guard.rs`），声明驱动
//! `relay`，程序是拷过去的量尺本身，带参数 [`ARG`] 起来（`driver/extension.rs`）。配置另加两个窗口大的模型：内置驱动的
//! `perf/big`（同一个假服务）、扩展驱动的 `perfx/big`，免得 1 MB 的请求到了压缩线。
//!
//! - **首字时延**：两个会话各先垫上四句、每句约 250 KB 的话（协议一行最长 1 MiB），之后每次请求约 1 MB；再各说 `turns` 轮，记
//!   说出去到推来第一段 `model.delta` 的字。比两边的 p95。
//! - **流式的 CPU**：两边都在一秒里匀着交 `pieces` 段：内置的照假服务切成 `pieces` 块、一个事件隔一下写（[`Fake::pace`]），扩展的照记号
//!   [`PACE`]；各说 `turns` 轮，记每一轮核心进程花的 CPU，比两边的中位数。一秒的流多花多少毫秒，就是多用了百分之几的一个核。

mod extension;

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::cpu::threads;
use crate::fake::Fake;
use crate::measure::subscribe;
use crate::measure::tools::guard::{enable_as, install_as};
use crate::rpc::Rpc;
use crate::sandbox::Sandbox;

pub use extension::{ARG, serve};

/// 扩展包的编号。
const PACKAGE: &str = "perf-driver";

/// 扩展照这个记号在一秒里匀着交增量；没有的当场交一段。
pub const PACE: &str = "perf-pace";

/// 垫几句、每句多少字节：协议一行最长 1 MiB，分几句垫到约 1 MB。
const PADDING: (usize, usize) = (4, 250_000);

/// 量到的（以前的原始数据没有这一份）。
#[derive(Debug, Clone, Default)]
pub struct Driven {
    /// 内置驱动：说出去到第一段增量推来（毫秒），一轮一个。
    pub builtin_first: Vec<f64>,
    /// 扩展驱动：同上。
    pub extension_first: Vec<f64>,
    /// 内置驱动交 `pieces` 段的一轮核心花的 CPU（毫秒）。读不到的平台是空的。
    pub builtin_cpu: Vec<f64>,
    /// 扩展驱动在一秒里交 `pieces` 段的一轮核心花的 CPU（毫秒）。
    pub extension_cpu: Vec<f64>,
    /// 一轮几段。
    pub pieces: usize,
}

/// 往数据根装上当驱动的扩展、配上两个窗口大的模型。核心起来以前调。
///
/// # Errors
///
/// 写不进、拷不了量尺自己。
pub fn install(sandbox: &Sandbox) -> Result<(), String> {
    install_as(
        &sandbox.root,
        (PACKAGE, ARG),
        "model.driver",
        "[[driver]]\nname = \"relay\"\nkey = \"none\"\n",
    )?;
    let path = sandbox.root.config_file();
    let mut config = std::fs::read_to_string(&path).map_err(|e| format!("系统配置读不了：{e}"))?;
    config.push_str(
        "\n[providers.perf.models.big]\nwindow = 4000000\n\n[providers.perfx]\ndriver = \"perf-driver/relay\"\n\n[providers.perfx.models.big]\nwindow = 4000000\n",
    );
    std::fs::write(&path, config).map_err(|e| format!("系统配置写不进：{e}"))
}

/// 量一遍（见模块的说明）：`turns` 轮，一轮 `pieces` 段。量完假模型换回一块。
///
/// # Errors
///
/// 起不来、连不上、扩展一直没在跑、哪一句没说成。
pub async fn driven(
    sandbox: &Sandbox,
    fake: &mut Fake,
    turns: usize,
    pieces: usize,
) -> Result<Driven, String> {
    let running = sandbox.start()?;
    let mut rpc = Rpc::connect(&sandbox.root).await?;
    let outcome = async {
        enable_as(&mut rpc, PACKAGE, "model.driver").await?;
        let builtin = open(&mut rpc, sandbox, "perf/big").await?;
        let extension = open(&mut rpc, sandbox, "perfx/big").await?;
        let filler = "perf filler ".repeat(PADDING.1 / 12);
        for session in [&builtin, &extension] {
            for n in 0..PADDING.0 {
                send(&mut rpc, session, &format!("padding {n}: {filler}")).await?;
            }
        }
        let mut driven = Driven {
            pieces,
            ..Driven::default()
        };
        for n in 0..turns {
            let marker = format!("perf-first-{n:03}: keep going.");
            driven
                .builtin_first
                .push(send(&mut rpc, &builtin, &marker).await?);
            driven
                .extension_first
                .push(send(&mut rpc, &extension, &marker).await?);
        }
        // 两边都在一秒里匀着交 `pieces` 段：真模型的回答是匀着来的，每一段都叫醒核心一次（头一回 CI 上内置的一次写完、扩展的匀着
        // 交，多出来的大半是叫醒的次数，不是扩展这一跳）。
        fake.split_into(pieces);
        fake.pace(Some(
            Duration::from_secs(1) / u32::try_from(pieces).unwrap_or(1),
        ));
        for n in 0..turns {
            let marker = format!("perf-cpu-{n:03}: keep going.");
            if let Some(cpu) = spent(&mut rpc, running.pid(), &builtin, &marker).await? {
                driven.builtin_cpu.push(cpu);
            }
            let paced = format!("{PACE} {n:03}: keep going.");
            if let Some(cpu) = spent(&mut rpc, running.pid(), &extension, &paced).await? {
                driven.extension_cpu.push(cpu);
            }
        }
        Ok::<Driven, String>(driven)
    }
    .await;
    fake.split_into(1);
    fake.pace(None);
    drop(rpc);
    sandbox.stop(running)?;
    outcome
}

/// 在工作区里造一个用模型 `model` 的会话（记忆关着），订阅它，交回编号。
async fn open(rpc: &mut Rpc, sandbox: &Sandbox, model: &str) -> Result<String, String> {
    let created = rpc
        .call(
            "session.create",
            json!({"cwd": sandbox.work.to_string_lossy(), "memory": crate::measure::FORGETFUL, "model": model}),
        )
        .await?;
    let session = created["session"]
        .as_str()
        .ok_or("session.create 的回应里没有编号")?
        .to_string();
    subscribe(rpc, &session).await?;
    Ok(session)
}

/// 说一句 `text`，等推来第一段正文的增量、等这一轮结束：交回说出去到第一段增量的毫秒数。
async fn send(rpc: &mut Rpc, session: &str, text: &str) -> Result<f64, String> {
    let began = Instant::now();
    rpc.call("session.send", json!({"session": session, "text": text}))
        .await?;
    let first = rpc.first_delta(session).await?;
    rpc.turn_ended(session).await?;
    Ok(first.saturating_duration_since(began).as_secs_f64() * 1000.0)
}

/// 说一句 `text`，交回这一轮核心进程（`pid`）花的 CPU（毫秒）；读不到的平台没有。
async fn spent(rpc: &mut Rpc, pid: u32, session: &str, text: &str) -> Result<Option<f64>, String> {
    let before = threads(pid);
    send(rpc, session, text).await?;
    Ok(before
        .zip(threads(pid))
        .map(|(before, after)| after - before))
}

/// 推来的一条是不是 `session` 的一段正文增量（`model.delta` 带 `text` 的）。
pub fn is_text_delta(push: &Value, session: &str) -> bool {
    push["method"] == "event"
        && push["params"]["session"] == session
        && push["params"]["event"]["kind"] == "model.delta"
        && push["params"]["event"]["body"]["text"].is_string()
}

#[cfg(test)]
mod tests;
