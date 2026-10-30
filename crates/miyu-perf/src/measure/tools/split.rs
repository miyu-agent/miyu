//! 把一步拆成几段（施工 V-29）：不改核心，照核心自己留下的时刻。
//!
//! - 会话日志：带这次调用的 `message.assistant` 的 `at`（actor 收到「说完了」的那一刻，`actor/back.rs`），这次调用的
//!   `tool.result` 的 `at`（结果交进内核的那一刻）。内核给调用另起编号（`call_8_1` 这样），供应商给的编号（剧本里的）
//!   收在那一块的 `private` 里：照它认出内核的编号，再找结果。
//! - 运行日志（`state/logs/core.log`）：这个会话的每一行。出厂的 `info` 有 `running`（判过权限、派出去之前）、`ran`
//!   （actor 收到工具做完）、`request`（交给请求模型的端口）；`MIYU_LOG=debug` 另有每一条输入、每一个动作一行
//!   （`input kind=…`、`action kind=…`），拆得更细，记日志本身也多花一点。
//!
//! 时刻都只到毫秒；每一处取它在一步里头一回出现的时刻，减去假模型写完回答的那一刻，几十次取平均。墙上时间和日志里的
//! 时刻同一台机器、同一个时钟，运行日志是本机时区，照核心起来时记的 `tz` 换算。

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use miyu_kernel::id::SessionId;
use miyu_store::root::DataRoot;

use super::{Baseline, Step, Steps};

/// 一处时刻：叫什么，几步里出现过、平均在写完回答以后多少毫秒。
#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    /// 哪一处：会话日志的事件种类，或运行日志的那件事（带 `kind`、`tool` 的接在后面）。
    pub label: String,
    /// 几步里出现过。
    pub seen: usize,
    /// 平均在写完回答以后多少毫秒。
    pub mean: f64,
}

/// 一种调用拆开的样子。
#[derive(Debug, Clone, PartialEq)]
pub struct Split {
    /// 表里的名字。
    pub name: String,
    /// 几步。
    pub steps: usize,
    /// 照平均的先后排的各处；最后一处是假模型读全下一个请求头。
    pub marks: Vec<Mark>,
}

/// 日志里的时刻只到毫秒：一格多宽。
const BUCKET: f64 = 1.0;

/// 照会话日志、运行日志拆每一种（对照也拆）；顺手查每次调用都成了。
///
/// # Errors
///
/// 日志读不了；哪次调用的结果不是 `ok`。
pub fn split(root: &DataRoot, baseline: &Baseline, kinds: &[Steps]) -> Result<Vec<Split>, String> {
    let lines = core_log(root);
    let tz = lines.iter().find_map(|line| offset(line)).unwrap_or(0.0);
    let parsed: Vec<Line> = lines
        .iter()
        .filter_map(|line| Line::parse(line, tz))
        .collect();
    let mut splits = vec![one(
        "什么都不做",
        &baseline.session,
        &baseline.steps,
        &[],
        &parsed,
    )];
    for kind in kinds {
        let events = events(root, &kind.session)?;
        for step in &kind.steps {
            check(&events, &kind.name, &step.call)?;
        }
        splits.push(one(
            &kind.name,
            &kind.session,
            &kind.steps,
            &events,
            &parsed,
        ));
    }
    Ok(splits)
}

/// 一种：每一步收齐各处的偏移，再平均。
fn one(name: &str, session: &str, steps: &[Step], events: &[Value], lines: &[Line]) -> Split {
    let mut sums: BTreeMap<String, (usize, f64)> = BTreeMap::new();
    for step in steps {
        let done = epoch_ms(step.done);
        let end = epoch_ms(step.arrived);
        let mut seen: BTreeMap<String, f64> = BTreeMap::new();
        // 记下的毫秒是截下来的：真的时刻在这一毫秒里，取中间。截下来的那一毫秒还没到写完回答的，不是这一步的。
        let mut note = |label: String, at: f64| {
            if at > done - BUCKET && at <= end {
                seen.entry(label).or_insert(at + BUCKET / 2.0 - done);
            }
        };
        let internal = internal_id(events, &step.call);
        for event in events
            .iter()
            .filter(|event| mentions_call(event, &step.call, internal.as_deref()))
        {
            if let (Some(kind), Some(at)) = (
                event["kind"].as_str(),
                event["at"].as_str().and_then(iso_ms),
            ) {
                note(format!("会话日志 {kind}"), at);
            }
        }
        // 运行日志从这一步的 `ended`（actor 收到这次回答说完了）起算：同一毫秒里还可能有上一步的 `request` 那几行。
        let mine: Vec<&Line> = lines
            .iter()
            .filter(|line| line.session == session)
            .collect();
        let first = mine
            .iter()
            .position(|line| line.label == "ended" && line.at > done - BUCKET);
        for line in first.map_or(&[][..], |first| &mine[first..]) {
            note(format!("运行日志 {}", line.label), line.at);
        }
        seen.insert("假模型读全下一个请求头".to_string(), step.ms);
        for (label, offset) in seen {
            let entry = sums.entry(label).or_insert((0, 0.0));
            entry.0 += 1;
            entry.1 += offset;
        }
    }
    let mut marks: Vec<Mark> = sums
        .into_iter()
        .map(|(label, (seen, sum))| Mark {
            label,
            seen,
            mean: sum / seen as f64,
        })
        .collect();
    marks.sort_by(|a, b| a.mean.total_cmp(&b.mean));
    Split {
        name: name.to_string(),
        steps: steps.len(),
        marks,
    }
}

/// 供应商给的编号 `call` 在内核里叫什么：`message.assistant` 里提到它的那一块调用的 `call_id`。
fn internal_id(events: &[Value], call: &str) -> Option<String> {
    events
        .iter()
        .filter(|event| event["kind"] == "message.assistant")
        .filter_map(|event| event["body"]["blocks"].as_array())
        .flatten()
        .find(|block| block["type"] == "tool_call" && block.to_string().contains(call))
        .and_then(|block| block["call_id"].as_str())
        .map(str::to_string)
}

/// 这条事件是不是这次调用的：`message.assistant` 里有它的调用（供应商的编号 `call`），或者 `tool.result` 是它的结果
/// （内核的编号 `internal`）。
fn mentions_call(event: &Value, call: &str, internal: Option<&str>) -> bool {
    match event["kind"].as_str() {
        Some("tool.result") => internal.is_some_and(|id| event["body"]["call_id"] == id),
        Some("message.assistant") => event["body"].to_string().contains(call),
        _ => false,
    }
}

/// 这次调用（供应商的编号 `call`）的结果是 `ok`：不是的说明量的不是工具正常干完活，报出来不白量。
fn check(events: &[Value], name: &str, call: &str) -> Result<(), String> {
    let internal = internal_id(events, call)
        .ok_or_else(|| format!("{name}：会话日志里没有 {call} 这次调用"))?;
    let result = events
        .iter()
        .find(|event| event["kind"] == "tool.result" && event["body"]["call_id"] == internal)
        .ok_or_else(|| format!("{name}：会话日志里没有 {call} 的结果"))?;
    match result["body"]["status"].as_str() {
        Some("ok") => Ok(()),
        status => Err(format!(
            "{name}：{call} 的结果是 {status:?}：{}",
            result["body"]["blocks"]
        )),
    }
}

/// 会话 `session` 的日志，每条写成 JSON。
fn events(root: &DataRoot, session: &str) -> Result<Vec<Value>, String> {
    let id = SessionId::parse(session).map_err(|e| format!("会话编号写坏了：{e}"))?;
    let events = miyu_store::log::read_events(&root.session_dir(&id))
        .map_err(|e| format!("会话日志读不了：{e}"))?;
    Ok(events
        .iter()
        .filter_map(|event| serde_json::from_str(&event.to_line()).ok())
        .collect())
}

/// 运行日志的每一行：轮换出去的（`core.log.1` 这些）先，老的在前。
fn core_log(root: &DataRoot) -> Vec<String> {
    let dir = root.state().join("logs");
    let mut lines = Vec::new();
    for n in (1..=5).rev() {
        lines.extend(read_lines(&dir.join(format!("core.log.{n}"))));
    }
    lines.extend(read_lines(&dir.join("core.log")));
    lines
}

fn read_lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .map(|text| text.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

/// 运行日志的一行：时刻（从 1970 年起的毫秒，UTC）、会话编号（没有的是空的）、标签。
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// 时刻。
    pub at: f64,
    /// 会话编号。
    pub session: String,
    /// 那件事，带 `kind=`、`tool=` 的接上它的值：`action append`、`running shell`。
    pub label: String,
}

impl Line {
    /// 读一行（格式见 `log.md` 第五条）；本机时区比 UTC 快 `tz` 分钟。读不懂的是 `None`。
    pub fn parse(text: &str, tz: f64) -> Option<Line> {
        let mut words = text.split_whitespace();
        let date = words.next()?;
        let time = words.next()?;
        let at = local_ms(date, time)? - tz * 60_000.0;
        let _level = words.next()?;
        let _source = words.next()?;
        let rest: Vec<&str> = words.collect();
        let (session, rest) = match rest.first() {
            Some(first) if SessionId::parse(first).is_ok() => (first.to_string(), &rest[1..]),
            _ => (String::new(), &rest[..]),
        };
        let what: Vec<&str> = rest
            .iter()
            .take_while(|word| !word.contains('='))
            .copied()
            .collect();
        let mut label = what.join(" ");
        for word in rest.iter().skip(what.len()) {
            if let Some(value) = word
                .strip_prefix("kind=")
                .or_else(|| word.strip_prefix("tool="))
            {
                label = format!("{label} {value}");
            }
        }
        Some(Line { at, session, label })
    }
}

/// `starting` 那一行的 `tz=+08:00`：本机比 UTC 快几分钟。
pub fn offset(text: &str) -> Option<f64> {
    let tz = text
        .split_whitespace()
        .find_map(|word| word.strip_prefix("tz="))?;
    let (sign, rest) = match tz.split_at_checked(1)? {
        ("+", rest) => (1.0, rest),
        ("-", rest) => (-1.0, rest),
        _ => return None,
    };
    let (hours, minutes) = rest.split_once(':')?;
    Some(sign * (hours.parse::<f64>().ok()? * 60.0 + minutes.parse::<f64>().ok()?))
}

/// `2026-09-25T07:04:07.905Z` 写成从 1970 年起的毫秒。
pub fn iso_ms(text: &str) -> Option<f64> {
    let text = text.strip_suffix('Z')?;
    let (date, time) = text.split_once('T')?;
    local_ms(date, time)
}

/// `2026-09-27`、`21:03:15.284` 当成 UTC 写成从 1970 年起的毫秒。
fn local_ms(date: &str, time: &str) -> Option<f64> {
    let mut day = date.split('-').map(str::parse::<i64>);
    let (year, month, d) = (day.next()?.ok()?, day.next()?.ok()?, day.next()?.ok()?);
    let mut clock = time.split(':');
    let (hours, minutes) = (
        clock.next()?.parse::<f64>().ok()?,
        clock.next()?.parse::<f64>().ok()?,
    );
    let seconds = clock.next()?.parse::<f64>().ok()?;
    let days = days_from_civil(year, month, d) as f64;
    Some(((days * 24.0 + hours) * 60.0 + minutes) * 60_000.0 + seconds * 1000.0)
}

/// 公历的年月日是 1970-01-01 以后第几天（Howard Hinnant 的 days_from_civil，`report.rs` 的 `date_of` 反过来）。
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// 墙上时间写成从 1970 年起的毫秒。
fn epoch_ms(at: SystemTime) -> f64 {
    at.duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests;
