//! 核心的运行时开几个工作线程（施工 CPU-1，`docs/blueprint/core.md`「起来的先后」第 7 条）。
//!
//! 原来写死两个，理由是「都是等 I/O」，没有实测。整机被一千多个线程挤满时 Linux 大体按线程分 CPU，两个线程大半时间在排队，
//! 长会话一轮从 0.65 秒掉到 10 秒（基准测试 bm2 的报告第 3 节）。项目主人 2026-11-12 定：默认照 CPU 核数，系统配置
//! `core.workers` 可以改，最少 2 个。
//!
//! 运行时在读配置以前就要建好，所以这里在建运行时以前同步读一次系统配置，只认这一项：读不到、写坏了、超范围的照核数，
//! 不报，后面正式读配置时照旧报（core1 2026-11-12 定）。改了要重启核心才算（`applies = restart`）。

use std::num::NonZeroUsize;

use miyu_config::parse::{Entry, parse};
use miyu_config::{Layer, Value};
use miyu_store::root::DataRoot;

use crate::serve::CoreSettings;

/// 这一项的键。
const KEY: &str = "core.workers";

/// 最少几个：写 1 的、取不到核数的照它。
const FEWEST: usize = 2;

/// 最多几个：和配置项的范围上限一样（`serve.rs` 的 `CoreSettings`）。核数比它多的机器也只开这么多。
const MOST: usize = 64;

/// 照数据根 `root` 的系统配置和本机的核数，定运行时开几个工作线程。
pub(crate) fn count(root: &DataRoot) -> usize {
    let written = miyu_store::config_file::read(&root.config_file())
        .ok()
        .flatten()
        .map_or(0, |read| written(&read.text));
    pick(written, std::thread::available_parallelism().ok())
}

/// 系统配置的字 `text` 里写的 `core.workers`：没写、写坏了（整份读不懂、类型不对、超范围、写错层）的是 0。
fn written(text: &str) -> i64 {
    let Ok(parsed) = parse(CoreSettings::ITEMS, Layer::System, text) else {
        return 0;
    };
    match parsed.entries.get(KEY) {
        Some(Entry {
            value: Value::Int(number),
            counts: true,
            ..
        }) => *number,
        _ => 0,
    }
}

/// 写的 `written` 大于 0 的照它，否则照核数 `cores`（取不到的照 [`FEWEST`]）；都夹到 [`FEWEST`] 到 [`MOST`]。
fn pick(written: i64, cores: Option<NonZeroUsize>) -> usize {
    match usize::try_from(written) {
        Ok(number) if number > 0 => number,
        _ => cores.map_or(FEWEST, NonZeroUsize::get),
    }
    .clamp(FEWEST, MOST)
}

#[cfg(test)]
mod tests;
