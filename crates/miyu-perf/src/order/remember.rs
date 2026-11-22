//! 排函数的剧本里抽记忆的那一步（施工 LW-14，`internal/blueprint/perf.md`「排函数」）：单独的一段里（`order/script.rs` 的
//! `remembered`）开着记忆的会话真的抽 [`ROUNDS`] 次，每一次都要假模型收到请求、核心把结果记进记忆日志才往下。
//!
//! 什么时候抽（`crates/miyu-session/src/actor/extract.rs` 开头）：会话闲了 `memory.extract_idle`（出厂 30 分钟，最少 60 秒）
//! 抽一次；一直不闲的会话结束了 `memory.extract_max_turns` 轮（出厂 20，最少 2）不等闲就抽。两条路都要这一段里至少有
//! `memory.extract_turns` 轮答了话（出厂 2），不够的什么也不发。剧本走不等闲的那一条：这一段的沙箱配置写
//! `extract_max_turns = 11`（[`SETTINGS`]），开着记忆的会话每说 [`TURNS`] 轮核心就抽一次，不用干等一分钟。真用的人说到二十轮
//! 走的也是这一条。
//!
//! 一次抽 11 轮、一共抽两次，是照长会话里真的抽取量出来的（2026-11-22，三十轮长会话的执行轨迹对清单）：只说两句、抽一次
//! 时，有 55 个函数录不到，它们零散地排在清单以外，真用时每执行到一个带进一片相邻的代码页，合起来多约 1.5 MB。这些函数
//! 分三类：一次抽的话超过 20 句时排序走的另一条路（11 轮是 22 句）；第二次抽时读已有的摘要、和已有的记忆对；抽完以后记下
//! 这一趟整理、关掉整理用的会话、清掉旧的整理会话。照现在的轮数录，只剩 9 个、10 kB。
//!
//! 合并只在头一次抽取以后等：离上次合并不到 `memory.merge_every`（出厂 16 小时）的，核心不再合并。
//!
//! 抽出来的要假模型回得像真的模型（[`crate::fake::Fake::organize`]）：回占地方的字，核心读不出结果，只记一行失败，记下、
//! 写关键词、合并那几路都走不到。

use std::path::Path;
use std::time::{Duration, Instant};

use crate::fake::{Fake, organizer};
use crate::sandbox::Sandbox;

/// 抽记忆那一段的沙箱配置加的：一直不闲的会话每结束 [`TURNS`] 轮就抽。
pub const SETTINGS: &str = "\n[memory]\nextract_max_turns = 11\n";

/// 说几轮抽一次：同 [`SETTINGS`] 里写的数。
pub const TURNS: usize = 11;

/// 一共抽几次。
pub const ROUNDS: usize = 2;

/// 等记忆日志最多多久：核心在模拟器里，同 `order/head.rs` 的 `PATIENCE`。
const PATIENCE: Duration = Duration::from_secs(600);

/// 多久看一次记忆日志。
const POLL: Duration = Duration::from_millis(200);

/// 记忆日志里这三样（`miyu-recall` 的 `memory.rs`）：抽到哪、换了关键词、合到哪。
const EXTRACTED: &str = "ext.memory.extracted";
const TAGGED: &str = "ext.memory.tagged";
const MERGED: &str = "ext.memory.merged";

/// 会话 `session` 说够一次抽取的轮数以后：等假模型收到抽取的请求、记忆日志里记下这个会话抽到了哪；再等写关键词、合并的
/// 请求和它们记下的那一条。`round` 是这个会话的第几次抽取（从 1 数），记忆日志里每一样要有这么多条才算。
///
/// # Errors
///
/// 哪一样等不到：报是在等什么。
pub async fn extracted(
    sandbox: &Sandbox,
    fake: &mut Fake,
    session: &str,
    round: usize,
) -> Result<(), String> {
    let dir = sandbox.root.path().join("root").join("memory");
    let steps = [
        ("抽取", organizer::EXTRACT, EXTRACTED, Some(session)),
        ("写关键词", organizer::TAG, TAGGED, None),
        ("合并", organizer::MERGE, MERGED, None),
    ];
    // 合并只等头一次：离上次合并不到 `memory.merge_every`（出厂 16 小时）的，核心不再合并。
    let steps = &steps[..if round == 1 { 3 } else { 2 }];
    for &(what, mark, kind, about) in steps {
        fake.request_with(mark)
            .await
            .map_err(|e| format!("第 {round} 次抽记忆：等{what}的请求：{e}"))?;
        logged(&dir, kind, about, round)
            .await
            .map_err(|e| format!("第 {round} 次抽记忆：{what}的结果：{e}"))?;
    }
    Ok(())
}

/// 等 `dir` 下的记忆日志里有 `times` 条 `kind`（`about` 给了的，这一行还要提到它）。
async fn logged(dir: &Path, kind: &str, about: Option<&str>, times: usize) -> Result<(), String> {
    let began = Instant::now();
    loop {
        let found = read_all(dir)?
            .lines()
            .filter(|line| recorded(line, kind, about))
            .count();
        if found >= times {
            return Ok(());
        }
        if began.elapsed() > PATIENCE {
            return Err(format!(
                "等了 {} 秒，{} 下的记忆日志里 {kind} 不到 {times} 条",
                PATIENCE.as_secs(),
                dir.display()
            ));
        }
        tokio::time::sleep(POLL).await;
    }
}

/// 记忆日志的一行是不是一条 `kind`，`about` 给了的还要提到它。只认事件的种类那一栏（`"kind":"<种类>"`），正文里提到这个
/// 词的不算。
fn recorded(line: &str, kind: &str, about: Option<&str>) -> bool {
    line.contains(&format!("\"kind\":\"{kind}\"")) && about.is_none_or(|about| line.contains(about))
}

/// `dir` 下所有文件的字接起来（一间的记忆日志只有几 kB）；还没建的当是空的。
fn read_all(dir: &Path) -> Result<String, String> {
    let mut text = String::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(text),
        Err(e) => return Err(format!("读不了 {}：{e}", dir.display())),
    };
    for entry in entries {
        let path = entry
            .map_err(|e| format!("读不了 {}：{e}", dir.display()))?
            .path();
        if path.is_dir() {
            text.push_str(&read_all(&path)?);
        } else if let Ok(bytes) = std::fs::read(&path) {
            text.push_str(&String::from_utf8_lossy(&bytes));
            text.push('\n');
        }
    }
    Ok(text)
}

#[cfg(test)]
mod tests;
