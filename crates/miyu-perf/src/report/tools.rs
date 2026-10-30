//! 工具一步的结果（施工 V-29）：`<日期>-tools-<系统>-<架构>[-<标签>].json`（原始数据，每一步的毫秒、CPU）和同名的 `.md`
//! （条件、每种的分位、对照、拆开的各段）。没有预算，不进闸门；只量这一场，别的几场的原始数据不动。

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::machine::Machine;
use crate::measure::tools::{Step, ToolSteps};
use crate::stats::percentile;

/// 量到的和条件。
pub struct ToolResults {
    /// 什么时候量的：`YYYY-MM-DD`（UTC）。
    pub date: String,
    /// 量的条件。
    pub machine: Machine,
    /// 量之前、量完的 `/proc/loadavg` 头三格：机器忙不忙。
    pub load: [Option<String>; 2],
    /// 文件名后面接的标签，没写的不接。
    pub tag: Option<String>,
    /// 量到的。
    pub steps: ToolSteps,
}

/// 写进 `out`，交回 `.md` 的路径。
///
/// # Errors
///
/// 建不了目录、写不进。
pub fn write(results: &ToolResults, out: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(out).map_err(|e| format!("建不了 {}：{e}", out.display()))?;
    let mut name = format!(
        "{}-tools-{}-{}",
        results.date, results.machine.os, results.machine.arch
    );
    if let Some(tag) = &results.tag {
        name = format!("{name}-{tag}");
    }
    let json = out.join(format!("{name}.json"));
    std::fs::write(&json, format!("{}\n", raw(results)))
        .map_err(|e| format!("写不进 {}：{e}", json.display()))?;
    let markdown = out.join(format!("{name}.md"));
    std::fs::write(&markdown, render(results, &name))
        .map_err(|e| format!("写不进 {}：{e}", markdown.display()))?;
    Ok(markdown)
}

/// 量之前、量完读的机器负载：`/proc/loadavg` 的头三格，读不到的是 `None`。
pub fn load() -> Option<String> {
    let text = std::fs::read_to_string("/proc/loadavg").ok()?;
    Some(
        text.split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// 毫秒留到微秒。
fn round(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn rounded(values: &[f64]) -> Value {
    values.iter().copied().map(round).collect()
}

/// 每一步写成 `[调用, 毫秒, 核心线程的 CPU, 连子进程的 CPU]`，读不到的 CPU 写 `null`。
fn steps_json(steps: &[Step]) -> Value {
    steps
        .iter()
        .map(|step| {
            json!([
                step.call,
                round(step.ms),
                step.cpu.threads.map(round),
                step.cpu.group.map(round)
            ])
        })
        .collect()
}

/// 原始数据。
pub fn raw(results: &ToolResults) -> Value {
    let steps = &results.steps;
    json!({
        "date": results.date,
        "machine": results.machine.to_json(),
        "load_before": results.load[0],
        "load_after": results.load[1],
        "samples": steps.samples,
        "sandbox": steps.sandbox,
        "scoped": steps.scoped,
        "log": steps.log,
        "baseline": {"steps_call_ms_cpu_threads_ms_cpu_group_ms": steps_json(&steps.baseline.steps),
            "ended_ms": rounded(&steps.baseline.ended), "resend_ms": rounded(&steps.baseline.resend),
            "sent_ms": rounded(&steps.baseline.sent)},
        "kinds": steps.kinds.iter().map(|kind| json!({"name": kind.name, "tool": kind.tool,
            "full": kind.full, "steps_call_ms_cpu_threads_ms_cpu_group_ms": steps_json(&kind.steps)}))
            .collect::<Vec<_>>(),
        "split": steps.split.iter().map(|split| json!({"name": split.name, "steps": split.steps,
            "marks": split.marks.iter().map(|mark| json!({"label": mark.label, "seen": mark.seen,
                "mean_ms": round(mark.mean)})).collect::<Vec<_>>()})).collect::<Vec<_>>(),
    })
}

/// `p50 / p95`，没有的写「—」。
fn spread(values: &[f64]) -> String {
    match (percentile(values, 50.0), percentile(values, 95.0)) {
        (Some(p50), Some(p95)) => format!("{p50:.2} / {p95:.2}"),
        _ => "—".to_string(),
    }
}

/// 一串步子里取一样的 p50，写成两位小数，没有的写「—」。
fn median_of(steps: &[Step], pick: fn(&Step) -> Option<f64>) -> String {
    let values: Vec<f64> = steps.iter().filter_map(pick).collect();
    percentile(&values, 50.0).map_or_else(|| "—".to_string(), |p50| format!("{p50:.2}"))
}

/// 表。
pub fn render(results: &ToolResults, name: &str) -> String {
    let steps = &results.steps;
    let machine = &results.machine;
    let or_dash = |value: Option<String>| value.unwrap_or_else(|| "—".to_string());
    let mut text = String::new();
    let mut line = |line: String| {
        text.push_str(&line);
        text.push('\n');
    };
    line(format!("# 工具一步 {name}"));
    line(String::new());
    line("照 `docs/blueprint/perf.md`「工具一步」量：假模型把带一个工具调用的回答写完（`[DONE]` 写出去）到读全带着结果的下一个请求头。".to_string());
    line(String::new());
    line("## 条件".to_string());
    line(String::new());
    line(format!(
        "- 机器：{}，{} 个逻辑核；系统 {} {}，内核 {}；{}；提交 {}",
        or_dash(machine.cpu.clone()),
        or_dash(machine.cores.map(|n| n.to_string())),
        machine.os,
        machine.arch,
        or_dash(machine.kernel.clone()),
        or_dash(machine.rustc.clone()),
        or_dash(machine.commit.clone())
    ));
    line(format!(
        "- 负载（`/proc/loadavg`）：量之前 {}，量完 {}",
        or_dash(results.load[0].clone()),
        or_dash(results.load[1].clone())
    ));
    line(format!(
        "- 每种调 {} 次另加头一次（不计）；沙盒：`{}`；连子进程的 CPU：{}；`MIYU_LOG`：{}",
        steps.samples,
        steps.sandbox,
        if steps.scoped {
            "核心在自己的 systemd scope 里，读 cgroup"
        } else {
            "不量"
        },
        steps
            .log
            .clone()
            .unwrap_or_else(|| "没设（出厂 info）".to_string())
    ));
    line(String::new());
    line("## 每种调用".to_string());
    line(String::new());
    line(
        "毫秒。「减底子」是 p50 减去对照里「收到 `turn.ended` 到读全下一个请求头」的 p50（下一节）；CPU 是这一段核心花的，p50。".to_string(),
    );
    line(String::new());
    line("| 调用 | 工具 | 沙盒 | p50 / p95 | 减底子 | 核心线程 CPU | 连子进程 CPU |".to_string());
    line("|---|---|---|---|---|---|---|".to_string());
    let base: Vec<f64> = steps.baseline.steps.iter().map(|step| step.ms).collect();
    let base_p50 = percentile(&steps.baseline.resend, 50.0);
    line(format!(
        "| 什么都不做 | — | — | {} | — | {} | {} |",
        spread(&base),
        median_of(&steps.baseline.steps, |step| step.cpu.threads),
        median_of(&steps.baseline.steps, |step| step.cpu.group)
    ));
    for kind in &steps.kinds {
        let values: Vec<f64> = kind.steps.iter().map(|step| step.ms).collect();
        let minus = match (percentile(&values, 50.0), base_p50) {
            (Some(p50), Some(base)) => format!("{:.2}", p50 - base),
            _ => "—".to_string(),
        };
        let sandboxed = match (kind.tool.as_str(), kind.full) {
            ("shell", false) => "开",
            ("shell", true) => "关",
            _ => "—",
        };
        line(format!(
            "| {} | `{}` | {} | {} | {} | {} | {} |",
            kind.name,
            kind.tool,
            sandboxed,
            spread(&values),
            minus,
            median_of(&kind.steps, |step| step.cpu.threads),
            median_of(&kind.steps, |step| step.cpu.group)
        ));
    }
    line(String::new());
    line("## 对照拆开".to_string());
    line(String::new());
    line(format!(
        "回答写完到量尺收到 `turn.ended` {}（回合结束落了盘、同步了才推）；收到 `turn.ended` 到读全下一个请求头 {}（量尺再说一句、核心收下、组装、发出：底子）；另记收到 `turn.ended` 到 `session.send` 的回应 {}（这一句同步了才回，请求常常先到）。p50 / p95，毫秒。",
        spread(&steps.baseline.ended),
        spread(&steps.baseline.resend),
        spread(&steps.baseline.sent)
    ));
    line(String::new());
    line("## 一步里的各处".to_string());
    line(String::new());
    line("每一处在回答写完以后平均多少毫秒头一回出现（日志的时刻只到毫秒、取那一毫秒的中间，几十步取平均；运行日志从这一步的 `ended` 起）；括号里是几步里见到。".to_string());
    line(String::new());
    for split in &steps.split {
        let mut marks = String::new();
        for mark in &split.marks {
            let seen = match mark.seen == split.steps {
                true => String::new(),
                false => format!("（{}/{}）", mark.seen, split.steps),
            };
            #[expect(clippy::let_underscore_must_use, reason = "写进 String 不会失败")]
            let _ = write!(marks, "{} **{:.2}**{seen} → ", mark.label, mark.mean);
        }
        line(format!(
            "- {}：{}",
            split.name,
            marks.trim_end_matches(" → ")
        ));
    }
    text
}
