//! 不许前台 `sleep` 干等（项目主人 2026-10-26 定）：后台任务办完会自己回报，她前台 `sleep` 等它只是白白占着这一轮。
//!
//! 只拒开头就睡的（项目主人 2026-11-27 定，施工 SH-3）：照顶层把命令切成几段（`;`、`&&`、`||`、换行、单个 `&`；引号、反斜杠、
//! 括号、花括号里的不算），第一段就是前台的 `sleep <时长>`（PowerShell 是 `Start-Sleep`），时长合起来不短于 [`SHORTEST`]，整条
//! 不跑：整条只有这一个 `sleep` 的，和 `sleep 5; curl …`、`sleep 10 && check` 这种先睡再看的。`sleep` 不在第一段的
//! （`postfix start; sleep 2; postfix status`，多半是在等服务起来）照跑；原来哪一段是 `sleep` 都拒，这些也拦了。管道里的、循环体里的
//! （`do sleep 1; done` 那一段开头是 `do`）、放到后台的（`sleep 5 &`）、写不出时长的照跑；时长写成变量这类看不出多长的当长的。
//! 2 秒的门槛和切段的认法是主会话定的技术细节。

use std::time::Duration;

/// 不短于这么久的算干等：一两秒以内的短停（让上一步落盘、限速）照跑。
const SHORTEST: Duration = Duration::from_secs(2);

/// 这条命令是不是开头就前台干等。
pub(super) fn waits(command: &str) -> bool {
    segments(command.trim())
        .first()
        .is_some_and(|(first, background)| !background && sleeps_long(first))
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

/// 这一段就是 `sleep <时长…>`（或 `Start-Sleep`），合起来不短于 [`SHORTEST`]。接着管道的（`sleep 3 | cat`）不算。
fn sleeps_long(segment: &str) -> bool {
    let mut words = segment.split_whitespace();
    let Some(first) = words.next() else {
        return false;
    };
    let rest: Vec<&str> = words.collect();
    if rest.iter().any(|word| word.contains('|')) {
        return false;
    }
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

/// 开头是 `sleep` 的（照 Claude Code）：到时照旧杀，不挪到后台（施工 7-12）。PowerShell 的 `Start-Sleep` 也算（不分大小写，
/// 施工 WN-1）。
pub(super) fn sleeps(command: &str) -> bool {
    command
        .split_whitespace()
        .next()
        .is_some_and(|first| first == "sleep" || first.eq_ignore_ascii_case("start-sleep"))
}

/// 她写的 `timeout`：没写、写 0 的按默认，超过上限的按上限。
pub(super) fn limit(timeout: Option<u64>) -> u64 {
    match timeout {
        None | Some(0) => super::DEFAULT,
        Some(timeout) => timeout.min(super::MAX),
    }
}

#[cfg(test)]
mod tests;
