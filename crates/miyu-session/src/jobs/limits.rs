//! 一条后台命令的上限（施工 7-12 下，照 Claude Code）：最多跑多久（`jobs.max_running`，默认 30 分钟、最多 2 小时），输出
//! 最多多少字节（5 GB，不给改）。到了整组停掉，报 `timed_out`、`too_large`（`run.rs`）。

use std::time::Duration;

use crate::config::TurnConfig;
use crate::settings::JobSettings;

/// 输出最多多少字节（照 Claude Code）。
pub(crate) const OUTPUT_LIMIT: u64 = 5 * 1024 * 1024 * 1024;

/// 一条后台命令的上限。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct JobLimits {
    /// 最多跑多久。
    pub(crate) running: Duration,
    /// 输出最多多少字节。
    pub(crate) output: u64,
}

impl JobLimits {
    /// 照这一轮的配置 `config`。
    pub(crate) fn of(config: &TurnConfig) -> JobLimits {
        JobLimits {
            running: JobSettings::from(&config.resolved.values()).max_running,
            output: OUTPUT_LIMIT,
        }
    }
}
