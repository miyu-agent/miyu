//! 汇率（施工 UC-1，`docs/blueprint/models.md`「怎么走」第九条第 8 条）：金额照它换成 `usage.currency`。以美元为中转：一天
//! 一个币种存一个「1 美元换多少」，A 换 B 是 `金额 ÷ USD→A × USD→B`，美元自己是 1；同一笔照它发生那天（UTC）的汇率换。
//!
//! - 缓存（`cache.rs`）：`state/rates.db`，派生的：坏了、版本不对删掉重建，重取就是。
//! - 取（`book.rs`）：缓存里有的直接用；没有的交给装上的取汇率的端口（[`RateSource`]，上层用 HTTP 实现），同时缺的并成
//!   一次、同一个缺口在取的跟着等，最多等 [`WAIT`]；等不到的这一次换不成，取回来照样记下。暂定的（当天的、兜底源的、
//!   交回的日子比要的早的）过了 6 小时或者那天过完再重取，重取时照用旧的、不等。
//! - 取汇率只交币种和日子，不交任何用量（隐私，核心主会话 2026-10-24 定）。

mod book;
mod cache;

pub use book::{Fetched, RateBook, RateSource, Source, WAIT};
pub use cache::FILE;

/// 一个 UTC 日子：从 1970-01-01 数起第几天。
pub type Day = i64;

/// 一天，毫秒。
pub(crate) const DAY_MS: i64 = 86_400_000;

/// 美元：中转的币种，汇率是 1。
pub const USD: &str = "USD";

/// 毫秒时刻 `ms` 是哪个 UTC 日子。
pub fn day_of(ms: i64) -> Day {
    ms.div_euclid(DAY_MS)
}

#[cfg(test)]
mod tests;
