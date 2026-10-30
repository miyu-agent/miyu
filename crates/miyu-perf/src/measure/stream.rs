//! 流式一轮（施工 V-22）：真模型的回答是一点点来的，一轮几百个事件，核心每收一块都要走一遍（解析、过内核、推给头）。
//! 别的几场假模型一次回一整块，量不出这一截。
//!
//! 在大会话上（一万条事件，每一块都要做的活里有跟着历史长的，这里才看得出）：重启核心、订阅它，先说一句不量（载入以后头
//! 一次从头投影），再说几轮，回答切成 `pieces` 块；每一轮记核心进程花了多少 CPU、一轮多久。

use crate::cpu::threads;
use crate::fake::Fake;
use crate::measure::{say, subscribe};
use crate::rpc::Rpc;
use crate::sandbox::Sandbox;

/// 量到的。
#[derive(Debug, Default)]
pub struct Streamed {
    /// 回答切成几块。
    pub pieces: usize,
    /// 一轮一个：核心进程花的 CPU（毫秒，各线程加起来）。读不到的平台是空的。
    pub cpu: Vec<f64>,
    /// 一轮一个：说出去到这一轮结束（毫秒）。
    pub turn: Vec<f64>,
}

/// 在会话 `session`（订阅时载入）上说 `turns` 轮，回答切成 `pieces` 块。量完假模型换回一块。
///
/// # Errors
///
/// 起不来、连不上、哪一句没说成。
pub async fn streamed(
    sandbox: &Sandbox,
    fake: &mut Fake,
    session: &str,
    turns: usize,
    pieces: usize,
) -> Result<Streamed, String> {
    let running = sandbox.start()?;
    let mut rpc = Rpc::connect(&sandbox.root).await?;
    subscribe(&mut rpc, session).await?;
    fake.split_into(pieces);
    let mut streamed = Streamed {
        pieces,
        ..Streamed::default()
    };
    let outcome = async {
        say(&mut rpc, fake, session, "perf-stream-warm").await?;
        for n in 0..turns {
            let before = threads(running.pid());
            let said = say(&mut rpc, fake, session, &format!("perf-stream-{n:03}")).await?;
            if let (Some(before), Some(after)) = (before, threads(running.pid())) {
                streamed.cpu.push(after - before);
            }
            streamed.turn.push(said.turn);
        }
        Ok::<(), String>(())
    }
    .await;
    fake.split_into(1);
    drop(rpc);
    sandbox.stop(running)?;
    outcome.map(|()| streamed)
}
