//! `miyu schedule` 印的样子和读的时刻、间隔（`docs/blueprint/cli/schedule.md`「样子」）：一项一行，编号、什么时候、停着、下一次、
//! 上一次、跑在哪、那句话，两个空格隔开；时刻照本机时区印到分钟。纯逻辑，时区由调的一方交。

use jiff::Timestamp;
use jiff::civil::DateTime;
use jiff::tz::TimeZone;
use serde_json::Value;

use crate::language::Language;

/// 那句话最多印几个字，多的截掉接 `…`。
const PROMPT: usize = 40;

/// 一项一行。
pub(crate) fn lines(listed: &[Value], language: &Language, zone: &TimeZone) -> Vec<String> {
    listed
        .iter()
        .map(|item| line(item, language, zone))
        .collect()
}

/// 一项：编号、什么时候、停着、下一次、上一次、跑在哪、那句话；没有的不写。
pub(crate) fn line(item: &Value, language: &Language, zone: &TimeZone) -> String {
    let mut parts = vec![item["id"].as_str().unwrap_or_default().to_string()];
    parts.push(when(&item["when"], language, zone));
    if let Some(kind) = item["loop"].as_str() {
        parts.push(language.schedule_loop(kind).to_string());
    }
    if item["paused"].as_bool() == Some(true) {
        parts.push(language.schedule_paused_mark().to_string());
    }
    if let Some(next) = item["next"].as_str() {
        parts.push(language.schedule_next(&local(next, zone)));
    }
    if let Some(at) = item["last"]["at"].as_str() {
        let result = item["last"]["result"].as_str().unwrap_or_default();
        parts.push(language.schedule_last(&local(at, zone), &language.schedule_result(result)));
    }
    parts.push(target(&item["target"], language));
    let prompt = item["prompt"].as_str().unwrap_or_default();
    parts.push(language.schedule_prompt(&cut(prompt)));
    parts.join("  ")
}

/// 什么时候：`cron 0 8 * * *（Asia/Tokyo）`、`at 2026-10-28 08:00`、`every 10m`。
fn when(when: &Value, language: &Language, zone: &TimeZone) -> String {
    if let Some(cron) = when["cron"].as_str() {
        let once = when["recurring"].as_bool() == Some(false);
        return language.schedule_cron(cron, when["zone"].as_str(), once);
    }
    if let Some(at) = when["at"].as_str() {
        return format!("at {}", local(at, zone));
    }
    match when["every"].as_u64() {
        Some(seconds) => format!("every {}", spoken(seconds)),
        None => String::new(),
    }
}

/// 跑在哪：已有的会话，或者每次新开（写了人格、预设、目录的跟着写）。
fn target(target: &Value, language: &Language) -> String {
    if let Some(session) = target["session"].as_str() {
        return language.schedule_on_session(session);
    }
    let new = &target["new"];
    let persona = match new.get("persona") {
        Some(Value::Null) => Some(None),
        Some(persona) => persona.as_str().map(Some),
        None => None,
    };
    language.schedule_on_new(persona, new["preset"].as_str(), new["cwd"].as_str())
}

/// 核心的时刻照本机时区印到分钟；读不懂的照原样。
fn local(text: &str, zone: &TimeZone) -> String {
    text.parse::<Timestamp>().map_or_else(
        |_| text.to_string(),
        |at| {
            at.to_zoned(zone.clone())
                .strftime("%Y-%m-%d %H:%M")
                .to_string()
        },
    )
}

/// 截到 [`PROMPT`] 个字，换行换成空格。
fn cut(prompt: &str) -> String {
    let flat: String = prompt
        .chars()
        .map(|c| if c == '\n' { ' ' } else { c })
        .collect();
    match flat.chars().count() > PROMPT {
        true => format!("{}…", flat.chars().take(PROMPT).collect::<String>()),
        false => flat,
    }
}

/// 秒数说成最大的整单位：`1d`、`2h`、`10m`、`90s`。
fn spoken(seconds: u64) -> String {
    match seconds {
        0 => "0s".to_string(),
        s if s % 86_400 == 0 => format!("{}d", s / 86_400),
        s if s % 3_600 == 0 => format!("{}h", s / 3_600),
        s if s % 60 == 0 => format!("{}m", s / 60),
        s => format!("{s}s"),
    }
}

/// `--every`：纯秒数，或者数字接 `s`、`m`、`h`、`d`；读不懂的、溢出的没有。
pub fn interval(text: &str) -> Option<u64> {
    let text = text.trim();
    let (number, unit) = match text.char_indices().last()? {
        (at, c) if c.is_ascii_alphabetic() => (&text[..at], c.to_ascii_lowercase()),
        _ => (text, 's'),
    };
    if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let scale = match unit {
        's' => 1,
        'm' => 60,
        'h' => 3_600,
        'd' => 86_400,
        _ => return None,
    };
    number.parse::<u64>().ok()?.checked_mul(scale)
}

/// `--at`：带偏移的 RFC 3339 照它；不带偏移的（`2026-10-28T08:00`、`2026-10-28 08:00`）照 `zone` 读。交回核心认的写法
/// `2026-10-28T08:00:00.000Z`；读不懂的没有。
pub fn moment(text: &str, zone: &TimeZone) -> Option<String> {
    let text = text.trim();
    let at = match text.parse::<Timestamp>() {
        Ok(at) => at,
        Err(_) => text
            .parse::<DateTime>()
            .ok()?
            .to_zoned(zone.clone())
            .ok()?
            .timestamp(),
    };
    let utc = at.to_zoned(TimeZone::UTC);
    Some(format!(
        "{}.{:03}Z",
        utc.strftime("%Y-%m-%dT%H:%M:%S"),
        utc.subsec_nanosecond() / 1_000_000
    ))
}
