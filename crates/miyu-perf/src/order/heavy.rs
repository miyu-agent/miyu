//! 重场景（施工 LW-30，`internal/blueprint/perf.md`「排函数」第 5 条第 10 款）：照真实使用补全的九段，接在抽记忆那一段后面，
//! 各自成段、日志依次排在核心清单的最后。只在带了 `--head` 的录法里录（`cargo xtask dist` 对着发行包目录录的那一遍，和 macOS
//! 打包时带探针跑的那一轮）。
//!
//! 为什么要录：以前的剧本只是「一个人用终端界面」。2026-11-23 在一台实际使用的机器上看（装着扩展、通讯平台、知识库），核心
//! 代码段里在内存的有 17.9 MB，清单排好的前 8 MiB 以外还有约 9.5 MB；在沙盒里照这样用一遍（扩展、平台桥、多会话、知识库、
//! 记忆整理、MCP、定时任务、抓网页、软件包），常驻的代码 22.4 MB，清单外 14.4 MB，可是模拟器里录到的、真正执行到的清单外的
//! 函数只有 5.4 MB：多出来的是这些函数零散地排在清单外，每执行到一个就带进一片相邻的代码页。
//!
//! 先后照「越常用越靠前」（同上一天量的清单外执行到的量）：扩展和平台桥（真用的人核心一起来它们就在，核心起来时的握手、
//! 登记、挂接点这一截有 0.6 MB）、多会话、网页界面那样的头加用量和模型、知识库、记忆的整理、MCP、定时任务和循环、抓网页和
//! 搜索、软件包和命令行。每一段都是新起的核心：前一段新执行到的排在前面，后一段只接新执行到的。
//!
//! 和前面几段一样的规矩：新加的不常走的路径单独成段、排在最后，前面几段的清单不变（LW-14 的教训：抽记忆录在前面时，平常也用的
//! 函数被它抢先执行、排到一起，三个状态各多 0.26–0.45 MB）。
//!
//! 布置（`heavy/setup.rs`）：两个扩展、三个 MCP 服务都是量尺自己扮（`heavy/role.rs`、`heavy/mcp.rs`），网页代理、SearXNG、
//! MCP 网址服务和 OAuth 在量尺进程里（`heavy/web.rs`）。九段前先原生起一遍核心、等扩展和 MCP 服务都在跑再停掉：扩展的开关、
//! MCP 的目录缓存都是第二次起来以后的样子，和真用的人一样。

mod mcp;
mod plans;
mod role;
mod segments;
mod segments2;
mod setup;
mod steps;
mod web;

use std::path::Path;
use std::time::Instant;

pub use mcp::{ARG as MCP_ARG, serve as serve_mcp};
pub use role::{ARG as EXTENSION_ARG, serve as serve_extension};

use super::{Recorder, Trace};
use crate::args::Args;
use crate::fake::Fake;
use crate::order::script::head;
use crate::sandbox::{Launch, Sandbox};
use steps::Ctx;

/// 九段：名字、做什么。
#[derive(Debug, Clone, Copy)]
enum Segment {
    Extensions,
    Sessions,
    Web,
    Knowledge,
    Memory,
    Mcp,
    Schedules,
    Fetch,
    Packages,
}

/// 照先后录的九段（为什么是这个先后见模块开头）。
const SEGMENTS: [(Segment, &str); 9] = [
    (Segment::Extensions, "扩展和平台桥"),
    (Segment::Sessions, "多会话"),
    (Segment::Web, "网页界面那样的头、用量和模型"),
    (Segment::Knowledge, "知识库"),
    (Segment::Memory, "记忆的整理"),
    (Segment::Mcp, "MCP"),
    (Segment::Schedules, "定时任务和循环"),
    (Segment::Fetch, "抓网页和搜索"),
    (Segment::Packages, "软件包和命令行"),
];

/// 布置、原生先起一遍，再照 [`SEGMENTS`] 一段段录：交回九段录下的，照先后。核心照 `recorder` 放进模拟器或者带探针起，信任
/// `trust` 那份证书；每段说一行用了多久。
///
/// # Errors
///
/// 布置不成、起不来、哪一段哪一步没成（报是哪一段）。
pub async fn record(
    args: &Args,
    sandbox: &Sandbox,
    fake: &mut Fake,
    recorder: Recorder<'_>,
    trust: &Path,
) -> Result<Vec<Trace>, String> {
    let web = web::Web::start().await?;
    let dir = args.work.join("heavy");
    let placed = setup::place(sandbox, &dir, web.port)?;
    let home = dir.join("user");
    std::fs::create_dir_all(home.join("distro"))
        .map_err(|e| format!("建不了 {}：{e}", home.display()))?;
    let proxy = format!("http://127.0.0.1:{}", web.port);
    let native = Launch {
        trust: Some(trust),
        home: Some(&home),
        proxy: Some(&proxy),
        ..Launch::default()
    };
    eprintln!("排函数：重场景，原生先起一遍（扩展、MCP 服务起来）");
    let running = sandbox.start_with(native)?;
    let warmed = async {
        let mut rpc = head(sandbox).await?;
        segments::ready(&mut rpc).await
    }
    .await;
    sandbox.stop(running)?;
    warmed.map_err(|e| format!("重场景原生的那一遍：{e}"))?;
    let mut ctx = Ctx {
        sandbox,
        fake,
        placed: &placed,
        web: &web,
        home: &home,
        said: 0,
    };
    let mut traces = Vec::new();
    let all = Instant::now();
    for (n, (segment, name)) in SEGMENTS.into_iter().enumerate() {
        let at = recorder.at(&args.work, &format!("order-heavy-{}", n + 1));
        let launch = recorder.launch(native, &at, &args.miyu);
        eprintln!(
            "排函数：重场景第 {} 段「{name}」（{}）",
            n + 1,
            recorder.inside()
        );
        let began = Instant::now();
        let running = sandbox.start_with(launch)?;
        let trace = recorder.trace(&running, &at, &args.miyu);
        let done = run(segment, &mut ctx).await;
        sandbox.stop(running)?;
        traces.push(trace?);
        done.map_err(|e| format!("重场景第 {} 段「{name}」：{e}", n + 1))?;
        eprintln!(
            "排函数：重场景第 {} 段「{name}」用了 {:.0} 秒",
            n + 1,
            began.elapsed().as_secs_f64()
        );
    }
    eprintln!(
        "排函数：重场景九段合计 {:.0} 秒",
        all.elapsed().as_secs_f64()
    );
    Ok(traces)
}

/// 照 `segment` 做一段。
async fn run(segment: Segment, ctx: &mut Ctx<'_>) -> Result<(), String> {
    match segment {
        Segment::Extensions => segments::extensions(ctx).await,
        Segment::Sessions => segments::sessions(ctx).await,
        Segment::Web => segments::web(ctx).await,
        Segment::Knowledge => segments::knowledge(ctx).await,
        Segment::Memory => segments2::memory(ctx).await,
        Segment::Mcp => segments2::mcp(ctx).await,
        Segment::Schedules => segments2::schedules(ctx).await,
        Segment::Fetch => segments2::fetch(ctx).await,
        Segment::Packages => segments2::packages(ctx).await,
    }
}
