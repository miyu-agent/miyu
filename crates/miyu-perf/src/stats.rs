//! 分位数：一串量到的毫秒数，报 p50、p95、p99（`23-性能预算.md` 第一节）。

use std::time::Duration;

/// 一段时间写成毫秒，保留小数。
pub fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

/// 第 `p` 百分位，照最近秩：排好序的第 ⌈p/100 × n⌉ 个。空的是 `None`。
pub fn percentile(values: &[f64], p: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let rank = (p / 100.0 * sorted.len() as f64).ceil() as usize;
    sorted.get(rank.clamp(1, sorted.len()) - 1).copied()
}

/// 成对比的中位数（施工 FL-9）：同一次里交替量的两边 `first`、`second`，第几对照第几对算 `pair(前一边, 后一边)`。
/// 两边对数不齐的只算齐的那几对，算出来不是有限数的（除以零）那一对不算；一对都没有的没有。
pub fn paired_median(first: &[f64], second: &[f64], pair: impl Fn(f64, f64) -> f64) -> Option<f64> {
    let pairs: Vec<f64> = first
        .iter()
        .zip(second)
        .map(|(first, second)| pair(*first, *second))
        .filter(|value| value.is_finite())
        .collect();
    percentile(&pairs, 50.0)
}

/// 写进表里的一格：`p50 / p95`，没量的写「—」。
pub fn spread(values: &[f64]) -> String {
    match (percentile(values, 50.0), percentile(values, 95.0)) {
        (Some(p50), Some(p95)) => format!("{p50:.1} / {p95:.1}"),
        _ => "—".to_string(),
    }
}

#[cfg(test)]
mod tests;
