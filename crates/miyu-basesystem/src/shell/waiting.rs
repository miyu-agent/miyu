//! 不许前台 `sleep` 干等（项目主人 2026-10-26 定）：后台任务办完会自己回报，她前台 `sleep` 等它只是白白占着这一轮。
//!
//! 照顶层把命令切成几段（`;`、`&&`、`||`、换行、单个 `&`；引号、反斜杠、括号、花括号里的不算），有一段就是前台的
//! `sleep <时长>`（PowerShell 是 `Start-Sleep`），时长合起来不短于 [`SHORTEST`]，整条不跑。管道里的、循环体里的
//! （`do sleep 1; done` 那一段开头是 `do`）、放到后台的（`sleep 5 &`）、写不出时长的照跑；时长写成变量这类看不出多长的当长的。

use std::time::Duration;

/// 不短于这么久的算干等：一两秒以内的短停（让上一步落盘、限速）照跑。
const SHORTEST: Duration = Duration::from_secs(2);

/// 这条命令有没有前台干等的一段。
pub(super) fn waits(command: &str) -> bool {
    segments(command)
        .into_iter()
        .any(|(segment, background)| !background && sleeps_long(&segment))
}

/// 照顶层切开：每段的字，和它是不是放到了后台（后面跟着单个 `&`）。
fn segments(command: &str) -> Vec<(String, bool)> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let (mut single, mut double, mut escaped, mut depth) = (false, false, false, 0usize);
    let mut chars = command.chars().peekable();
    while let Some(c) = chars.next() {
        if escaped {
            escaped = false;
            current.push(c);
            continue;
        }
        match c {
            '\\' if !single => escaped = true,
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            '(' | '{' if !single && !double => depth += 1,
            ')' | '}' if !single && !double => depth = depth.saturating_sub(1),
            _ => {}
        }
        let top = !single && !double && depth == 0 && !escaped;
        let split = match c {
            ';' | '\n' if top => Some(false),
            '&' if top && chars.peek() == Some(&'&') => {
                chars.next();
                Some(false)
            }
            '&' if top => Some(true),
            '|' if top && chars.peek() == Some(&'|') => {
                chars.next();
                Some(false)
            }
            _ => None,
        };
        match split {
            Some(background) => segments.push((std::mem::take(&mut current), background)),
            None => current.push(c),
        }
    }
    segments.push((current, false));
    segments
}

/// 这一段就是 `sleep <时长…>`（或 `Start-Sleep`），合起来不短于 [`SHORTEST`]。
fn sleeps_long(segment: &str) -> bool {
    let mut words = segment.split_whitespace();
    let Some(first) = words.next() else {
        return false;
    };
    let rest: Vec<&str> = words.collect();
    if first == "sleep" {
        return total(&rest).is_none_or(|total| total >= SHORTEST);
    }
    if first.eq_ignore_ascii_case("start-sleep") {
        return start_sleep(&rest).is_none_or(|total| total >= SHORTEST);
    }
    false
}

/// `sleep` 的几个时长加起来（GNU 的写法：`5`、`0.5`、`1m`、`2h`、`1d`）；有一个看不出多长的交回 `None`，没写时长的是零。
fn total(args: &[&str]) -> Option<Duration> {
    args.iter().try_fold(Duration::ZERO, |sum, arg| {
        let (number, unit) = match arg.char_indices().last() {
            Some((at, unit)) if unit.is_ascii_alphabetic() => (&arg[..at], unit),
            _ => (*arg, 's'),
        };
        let seconds: f64 = number.parse().ok()?;
        let scale = match unit {
            's' => 1.0,
            'm' => 60.0,
            'h' => 3600.0,
            'd' => 86400.0,
            _ => return None,
        };
        (seconds.is_finite() && seconds >= 0.0)
            .then(|| sum + Duration::from_secs_f64(seconds * scale))
    })
}

/// `Start-Sleep` 的时长：`-Seconds 5`、`-Milliseconds 500`、`-s 5`、`-m 500`，或者直接写秒数。看不出的交回 `None`。
fn start_sleep(args: &[&str]) -> Option<Duration> {
    let mut args = args.iter();
    let mut found = None;
    while let Some(arg) = args.next() {
        let lower = arg.to_ascii_lowercase();
        let (value, millis) = if lower.starts_with("-m") {
            (args.next()?, true)
        } else if lower.starts_with("-s") || lower.starts_with("-d") {
            (args.next()?, false)
        } else {
            (arg, false)
        };
        let number: f64 = value.parse().ok()?;
        let seconds = if millis { number / 1000.0 } else { number };
        if !seconds.is_finite() || seconds < 0.0 {
            return None;
        }
        found = Some(Duration::from_secs_f64(seconds));
    }
    Some(found.unwrap_or(Duration::ZERO))
}

#[cfg(test)]
mod tests;
