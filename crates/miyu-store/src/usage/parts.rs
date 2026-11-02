//! 金额分四种（施工 UP-1，`docs/blueprint/models.md`「怎么走」第九条第 8 条「分四种」）：没命中的输入、读缓存、写缓存、输出各花了多少。
//!
//! - 照这一次请求自己记下的价格算（`model.called`、`usage.oneshot` 的 `cost.price`、`cost.multiplier`）：和那一次的金额同一份
//!   价格、同一个算法（`miyu_models::price`），以后目录更新、人改倍率，算出来的不变。不照现在的价格重算。
//! - 一种 = 那一种的用量 × 那一种的价 ÷ 1000000 × 倍率。用量是 0 的那一种就是准的 0，不管价格记没记；用量不是 0、价格又
//!   没记的，那一种不知道（核心主会话 2026-11-02 定：0 用量的 0 元是准数，不违反「算不准的钱不显示」）。
//! - 几笔加起来：哪一笔哪一种不知道，加起来的那一种也不知道；哪一笔整个没有四种（老的合计），加起来的整个没有
//!   （[`Parts::join`]）。算不准的钱不显示，不拼半截。

use serde::{Deserialize, Serialize};

use miyu_kernel::event::{Cost, Real, Usage};

/// 价格按每一百万 token 写。
const PER: f64 = 1_000_000.0;

/// 四种各花了多少，币种照那一笔的（换算以后照换成的）；不知道的没有。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parts {
    /// 没命中缓存的输入。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncached: Option<Real>,
    /// 读缓存。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<Real>,
    /// 写缓存。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<Real>,
    /// 输出（含思考）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<Real>,
}

impl Parts {
    /// 四种都是 0：加起来的起点。
    pub fn zero() -> Parts {
        let zero = Some(Real::new(0.0));
        Parts {
            uncached: zero,
            cache_read: zero,
            cache_write: zero,
            output: zero,
        }
    }

    /// 一次请求：用量 `usage` 照它记下的金额 `cost` 里的价格、倍率分四种。
    pub fn of(cost: &Cost, usage: &Usage) -> Parts {
        let multiplier = cost.multiplier.get();
        let part = |tokens: u64, rate: Option<Real>| match tokens {
            0 => Some(Real::new(0.0)),
            _ => rate.map(|rate| Real::new(tokens as f64 * rate.get() / PER * multiplier)),
        };
        Parts {
            uncached: part(usage.uncached, cost.price.input),
            cache_read: part(usage.cache_read, cost.price.cache_read),
            cache_write: part(usage.cache_write, cost.price.cache_write),
            output: part(usage.output, cost.price.output),
        }
    }

    /// 两份各种加起来：哪一份哪一种不知道，那一种不知道。
    #[must_use]
    pub fn plus(self, other: Parts) -> Parts {
        let add = |one: Option<Real>, other: Option<Real>| {
            one.zip(other)
                .map(|(one, other)| Real::new(one.get() + other.get()))
        };
        Parts {
            uncached: add(self.uncached, other.uncached),
            cache_read: add(self.cache_read, other.cache_read),
            cache_write: add(self.cache_write, other.cache_write),
            output: add(self.output, other.output),
        }
    }

    /// 每一种照 `f` 换（换算成别的币种）；不知道的照旧不知道。
    #[must_use]
    pub fn map(self, f: impl Fn(f64) -> f64) -> Parts {
        let each = |part: Option<Real>| part.map(|part| Real::new(f(part.get())));
        Parts {
            uncached: each(self.uncached),
            cache_read: each(self.cache_read),
            cache_write: each(self.cache_write),
            output: each(self.output),
        }
    }

    /// 两笔加起来：有一笔整个没有四种的，加起来的也没有。
    pub fn join(one: Option<Parts>, other: Option<Parts>) -> Option<Parts> {
        one.zip(other).map(|(one, other)| one.plus(other))
    }

    /// 四种都不知道。
    pub fn is_unknown(&self) -> bool {
        [
            self.uncached,
            self.cache_read,
            self.cache_write,
            self.output,
        ]
        .iter()
        .all(Option::is_none)
    }
}
