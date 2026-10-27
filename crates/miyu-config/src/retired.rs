//! 退役的键（施工 FL-1）：以前登记过、现在拆掉了的配置项。写过它们的配置照旧读得进：不进最终值，也不报 `unknown_key`，
//! 免得每次查配置都多一条警告。新写的（`config.set`）照不认识的拒。
//!
//! - `providers.<id>.compat.strict_tools`、`loading.tools.<name>`、`loading.mcp.<id>`：工具一律全量以后不起作用了（设计
//!   `25-工具加载.md` W10，2026-10-27 项目主人定）。

use crate::key::{Fit, fit};

/// 退役的几样，写法同配置项的键：占位照 [`crate::key`] 的规矩。
const RETIRED: &[&str] = &[
    "providers.<id>.compat.strict_tools",
    "loading.tools.<name>",
    "loading.mcp.<id>",
];

/// 照段拆好的键 `segments` 是不是退役了的。
pub(crate) fn retired(segments: &[String]) -> bool {
    RETIRED
        .iter()
        .any(|pattern| matches!(fit(pattern, segments), Fit::Yes(_)))
}

#[cfg(test)]
mod tests;
