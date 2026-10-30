//! 工具一步（施工 V-29）：模型发出一个工具调用以后，核心多久把结果送回模型。
//!
//! 假模型照剧本（`fake/script.rs`）回：一轮里一次接一次地调同一种工具，每次一个调用；量的是假模型把带调用的回答写完
//! （`[DONE]` 写出去）到读全带着这次结果的下一个请求头，同一段里核心花的 CPU（进程自己的线程；放进了自己的 scope 的再
//! 报连子进程的，`cpu.rs`）。每种的头一次不算（头一回跑某件工具、头一回拉起沙盒、文件还不在页缓存里），之后的照分位报。
//!
//! 「什么都不做」的对照：同样的回答（同一句正文）不带调用，这一轮就结束了，量尺收到 `turn.ended` 当场再说一句；量假模型
//! 写完回答到收到下一个请求，拆成写完到收到 `turn.ended`（回合结束先落盘、同步了才推）、收到 `turn.ended` 到读全下一个
//! 请求头（量尺来回一趟、核心收下这一句、组装、发出：底子）。工具一步不等同步（`tool.result` 只写不同步），比的是底子。
//!
//! 会话都关着记忆（`memory: off`）、不起标题（`titles: false`），照出厂预设（`full`）的工具面；`shell` 在工作区这一级
//! 关进沙盒，另有几种在完全放开下跑（沙盒关着），两边一比就是沙盒的开销。

pub mod calls;
pub mod split;

use std::path::Path;
use std::time::{Instant, SystemTime};

use serde_json::{Value, json};

use crate::cpu::{Cpu, Probe, group_of};
use crate::fake::script::{Script, ToolCall};
use crate::fake::{Answered, Arrival, Fake};
use crate::measure::subscribe;
use crate::rpc::{PATIENCE, Rpc};
use crate::sandbox::{Launch, Sandbox};
use crate::stats::ms;
use calls::{KINDS, Kind, Workspace};

/// 怎么量。
#[derive(Debug, Clone, Copy)]
pub struct Plan<'a> {
    /// 每种量几次（头一次另算，不计）。
    pub samples: usize,
    /// 拷进工作区的树。
    pub tree: &'a Path,
    /// 核心的 `MIYU_LOG`：拆开各段时写 `debug`（`split.rs`）。
    pub log: Option<&'a str>,
}

/// 量到的一步。
#[derive(Debug, Clone)]
pub struct Step {
    /// 这一步对应的调用编号；对照里是那一轮的记号。
    pub call: String,
    /// 回答写完到读全下一个请求头（毫秒）。
    pub ms: f64,
    /// 这一段核心花的 CPU。
    pub cpu: Cpu,
    /// 回答写完的墙上时间。
    pub done: SystemTime,
    /// 读全下一个请求头的墙上时间。
    pub arrived: SystemTime,
}

/// 一种调用量到的。
#[derive(Debug, Clone)]
pub struct Steps {
    /// 表里的名字。
    pub name: String,
    /// 工具名。
    pub tool: String,
    /// 在完全放开下跑的（沙盒关着）。
    pub full: bool,
    /// 会话编号：拆段时读它的日志。
    pub session: String,
    /// 每一步，头一次不在里面。
    pub steps: Vec<Step>,
}

/// 「什么都不做」的对照。
#[derive(Debug, Clone, Default)]
pub struct Baseline {
    /// 会话编号。
    pub session: String,
    /// 每一步：上一轮回答写完到这一轮的请求头。
    pub steps: Vec<Step>,
    /// 其中：回答写完到量尺收到 `turn.ended`。
    pub ended: Vec<f64>,
    /// 其中：收到 `turn.ended` 到读全下一个请求头（量尺再说一句、核心收下、组装、发出）：底子。
    pub resend: Vec<f64>,
    /// 另记：收到 `turn.ended` 到 `session.send` 的回应（核心落了盘、同步了才回，请求常常先到）。
    pub sent: Vec<f64>,
}

/// 整场量到的。
#[derive(Debug, Clone)]
pub struct ToolSteps {
    /// 握手回应里的 `sandbox`：这台机器上沙盒能不能用。
    pub sandbox: Value,
    /// 核心放进了自己的 scope：连子进程的 CPU 读得到。
    pub scoped: bool,
    /// 核心的 `MIYU_LOG`，没设的是 `None`。
    pub log: Option<String>,
    /// 每种量几次。
    pub samples: usize,
    /// 对照。
    pub baseline: Baseline,
    /// 每种调用。
    pub kinds: Vec<Steps>,
    /// 拆开的各段（`split.rs`）。
    pub split: Vec<split::Split>,
}

/// 量一遍：建工作区、拉起核心、先量对照再量每一种，停掉核心以后照日志拆段。
///
/// # Errors
///
/// 工作区建不了、核心起不来、哪一步没等到、哪次调用出了错。
pub async fn tool_steps(
    sandbox: &Sandbox,
    fake: &mut Fake,
    plan: Plan<'_>,
) -> Result<ToolSteps, String> {
    let workspace = Workspace::prepare(&sandbox.work.join("tools"), plan.tree)?;
    let scoped = has_systemd_run();
    let running = sandbox.start_with(Launch {
        scoped,
        log: plan.log,
    })?;
    let pid = running.pid();
    let mut rpc = Rpc::connect(&sandbox.root).await?;
    fake.probe(Some(Probe {
        pid,
        group: scoped.then(|| group_of(pid)).flatten(),
    }));
    let outcome = async {
        let baseline = baseline(&mut rpc, fake, &workspace, plan.samples).await?;
        let mut kinds = Vec::new();
        for (index, kind) in KINDS.iter().enumerate() {
            eprintln!("  {}", kind.name);
            kinds.push(steps(&mut rpc, fake, &workspace, plan.samples, index, kind).await?);
        }
        Ok::<_, String>((baseline, kinds))
    }
    .await;
    fake.probe(None);
    fake.script(None);
    let hello = rpc.hello["sandbox"].clone();
    drop(rpc);
    sandbox.stop(running)?;
    let (baseline, kinds) = outcome?;
    let split = split::split(&sandbox.root, &baseline, &kinds)?;
    Ok(ToolSteps {
        sandbox: hello,
        scoped,
        log: plan.log.map(str::to_string),
        samples: plan.samples,
        baseline,
        kinds,
        split,
    })
}

/// 有没有 `systemd-run`：有的核心放进自己的 scope，连子进程的 CPU 读得到。
fn has_systemd_run() -> bool {
    cfg!(target_os = "linux")
        && std::process::Command::new("systemd-run")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
}

/// 在工作区造一个关着记忆、不起标题的会话，`full` 的切到完全放开，订阅它。
async fn create(rpc: &mut Rpc, workspace: &Workspace, full: bool) -> Result<String, String> {
    let created = rpc
        .call(
            "session.create",
            json!({"cwd": workspace.dir.to_string_lossy(), "memory": crate::measure::FORGETFUL,
                "titles": false}),
        )
        .await?;
    let session = created["session"]
        .as_str()
        .ok_or("session.create 的回应里没有编号")?
        .to_string();
    if full {
        rpc.call(
            "session.set_permission_level",
            json!({"session": session, "level": "full"}),
        )
        .await?;
    }
    subscribe(rpc, &session).await?;
    Ok(session)
}

/// 等这个请求的回答写完。
async fn answered(mut arrival: Arrival) -> Result<(Arrival, Answered), String> {
    let done = tokio::time::timeout(PATIENCE, &mut arrival.answered)
        .await
        .map_err(|_| "假模型的回答一直没写完".to_string())?
        .map_err(|_| "假模型的回答没写完连接就断了".to_string())?;
    Ok((arrival, done))
}

/// 一步：回答 `done` 写完到请求 `arrival` 读全。
fn step(call: String, done: &Answered, arrival: &Arrival) -> Step {
    let cpu = match (arrival.cpu, done.cpu) {
        (Some(after), Some(before)) => after.since(&before),
        _ => Cpu::default(),
    };
    Step {
        call,
        ms: ms(arrival.at.saturating_duration_since(done.at)),
        cpu,
        done: done.wall,
        arrived: arrival.wall,
    }
}

/// 「什么都不做」：说 `samples + 1` 轮，每轮回同一句、不带调用；头一轮不算。
async fn baseline(
    rpc: &mut Rpc,
    fake: &mut Fake,
    workspace: &Workspace,
    samples: usize,
) -> Result<Baseline, String> {
    let session = create(rpc, workspace, false).await?;
    fake.script(Some(Script {
        marker: "perf-null-".to_string(),
        calls: Vec::new(),
    }));
    let mut baseline = Baseline {
        session: session.clone(),
        ..Baseline::default()
    };
    let mut last: Option<(Answered, Instant)> = None;
    for n in 0..=samples {
        let marker = format!("perf-null-{n:03}");
        rpc.call(
            "session.send",
            json!({"session": session, "text": format!("{marker}: go.")}),
        )
        .await?;
        let persisted = Instant::now();
        let (arrival, done) = answered(fake.request_with(&marker).await?).await?;
        if let Some((before, ended)) = &last {
            baseline.steps.push(step(marker, before, &arrival));
            baseline
                .ended
                .push(ms(ended.saturating_duration_since(before.at)));
            baseline
                .resend
                .push(ms(arrival.at.saturating_duration_since(*ended)));
            baseline
                .sent
                .push(ms(persisted.saturating_duration_since(*ended)));
        }
        rpc.turn_ended(&session).await?;
        last = Some((done, Instant::now()));
    }
    Ok(baseline)
}

/// 一种调用：一轮里调 `samples + 1` 次，头一次不算。
async fn steps(
    rpc: &mut Rpc,
    fake: &mut Fake,
    workspace: &Workspace,
    samples: usize,
    index: usize,
    kind: &Kind,
) -> Result<Steps, String> {
    let marker = format!("perf-tool-{index:02}");
    let calls: Vec<ToolCall> = (0..=samples)
        .map(|n| ToolCall {
            id: format!("perf_{index:02}_{n:03}"),
            name: kind.tool.to_string(),
            arguments: kind.args(workspace, n).to_string(),
        })
        .collect();
    let ids: Vec<String> = calls.iter().map(|call| call.id.clone()).collect();
    fake.script(Some(Script {
        marker: marker.clone(),
        calls,
    }));
    let session = create(rpc, workspace, kind.full).await?;
    rpc.call(
        "session.send",
        json!({"session": session, "text": format!("{marker}: go.")}),
    )
    .await?;
    let mut answers: Vec<(Arrival, Answered)> = Vec::new();
    for expected in 0..=samples + 1 {
        let (arrival, done) = answered(fake.request_with(&marker).await?).await?;
        if arrival.step != Some(expected) {
            return Err(format!(
                "{}：第 {expected} 个请求带着的结果对不上（{:?}）",
                kind.name, arrival.step
            ));
        }
        answers.push((arrival, done));
    }
    rpc.turn_ended(&session).await?;
    let steps = (1..=samples)
        .map(|n| step(ids[n].clone(), &answers[n].1, &answers[n + 1].0))
        .collect();
    Ok(Steps {
        name: kind.name.to_string(),
        tool: kind.tool.to_string(),
        full: kind.full,
        session,
        steps,
    })
}
