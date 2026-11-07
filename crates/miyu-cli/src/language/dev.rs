//! `miyu dev` 给人看的字（施工 MD-3，`docs/blueprint/cli/dev.md`「给人看的字」）：结果行不带句号；原因照核心的英文原话接在后面。

use super::Language;

impl Language {
    /// `driver-check` 一个驱动那一行：驱动、缓存类型、结果（`passed`、`failed`、`skipped`）。
    pub(crate) fn driver_checked(&self, driver: &str, cache: &str, outcome: &str) -> String {
        let said = match (self, outcome) {
            (Language::Chinese, "passed") => "通过",
            (Language::Chinese, "failed") => "没通过",
            (Language::Chinese, "skipped") => "跳过",
            (Language::English, "passed") => "passed",
            (Language::English, "failed") => "failed",
            (Language::English, "skipped") => "skipped",
            (_, other) => other,
        };
        match self {
            Language::Chinese => format!("{driver}（{cache}）{said}"),
            Language::English => format!("{driver} ({cache}) {said}"),
        }
    }

    /// `driver-check` 一份样本那一行：过了的、没过的。
    pub(crate) fn sample_checked(&self, sample: &str, ok: bool) -> String {
        match (self, ok) {
            (Language::Chinese, true) => format!("通过 {sample}"),
            (Language::Chinese, false) => format!("没通过 {sample}"),
            (Language::English, true) => format!("ok {sample}"),
            (Language::English, false) => format!("failed {sample}"),
        }
    }
}
