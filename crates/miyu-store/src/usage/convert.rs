//! 金额换算（施工 UC-1，`docs/blueprint/models.md`「怎么走」第九条第 8 条）：一组的金额照每一笔发生那天（UTC）的汇率换成
//! 一个币种加起来；哪一天哪个币种（或者要换成的那个币种）的汇率没有，那几笔照币种各加各的留在 `rest`。一笔都没换成的没有。
//!
//! 分四种的（施工 UP-1，[`Parts`]）照同样的汇率换，只加换成了的那几笔，所以四种加起来就是 `amount`；哪一笔（换没换成都算）
//! 整个没有四种的，这一组不给四种，哪一笔哪一种不知道的，这一组那一种不给。

use std::collections::BTreeMap;

use super::parts::Parts;
use super::rates::{Day, Source};

/// 一组里一个 UTC 日子、一个币种加起来的金额（施工 UC-1），连同分四种的（施工 UP-1）：换算照它。
#[derive(Debug, Clone, PartialEq)]
pub struct Daily {
    /// 哪个 UTC 日子。
    pub day: Day,
    /// 币种。
    pub currency: String,
    /// 金额。
    pub amount: f64,
    /// 分四种的；有一笔没记四种的（老的合计）没有。
    pub parts: Option<Parts>,
}

impl Daily {
    /// `day` 这一天、币种 `currency`，还什么都没加。
    pub fn new(day: Day, currency: String) -> Daily {
        Daily {
            day,
            currency,
            amount: 0.0,
            parts: Some(Parts::zero()),
        }
    }

    /// 加上一笔：金额 `amount`，分四种的 `parts`（没有的这一组以后都没有）。
    pub fn add(&mut self, amount: f64, parts: Option<Parts>) {
        self.amount += amount;
        self.parts = Parts::join(self.parts, parts);
    }
}

/// 换算好的一组。
#[derive(Debug, Clone, PartialEq)]
pub struct Converted {
    /// 换成的币种。
    pub currency: String,
    /// 换成以后加起来的。
    pub amount: f64,
    /// 没换成的，照币种各加各的，照币种代码的字母先后；都换成了的是空的。
    pub rest: Vec<(String, f64)>,
    /// 用到了兜底源的汇率：界面要注明来源。
    pub fallback: bool,
    /// 换成以后分四种的（施工 UP-1），只算换成了的那几笔；有一笔没记四种的没有。
    pub parts: Option<Parts>,
}

/// 把 `daily`（一组里按 UTC 日子、币种分开的金额）照 `rates`（1 美元换多少）换成 `target`。
pub(crate) fn convert(
    daily: &[Daily],
    target: &str,
    rates: &BTreeMap<(Day, String), (f64, Source)>,
) -> Option<Converted> {
    let mut amount = 0.0;
    let mut converted = false;
    let mut fallback = false;
    let mut rest: BTreeMap<String, f64> = BTreeMap::new();
    let mut parts = Some(Parts::zero());
    for daily in daily {
        let (currency, value) = (&daily.currency, daily.amount);
        // 这一笔换成 `target` 照什么换；换不成的没有。
        let rate: Option<(f64, f64)> = match currency == target {
            true => Some((1.0, 1.0)),
            false => match (
                rates.get(&(daily.day, currency.clone())),
                rates.get(&(daily.day, target.to_string())),
            ) {
                (Some((from, one)), Some((to, other))) if *from > 0.0 => {
                    fallback |= *one == Source::Fallback || *other == Source::Fallback;
                    Some((*from, *to))
                }
                _ => None,
            },
        };
        let each = match rate {
            Some((from, to)) => {
                amount += value / from * to;
                converted = true;
                daily.parts.map(|four| four.map(|part| part / from * to))
            }
            None => {
                *rest.entry(currency.clone()).or_default() += value;
                // 没换成的不进四种，可它不知道的那几种照样算这一组不知道。
                daily.parts.map(|four| four.map(|_| 0.0))
            }
        };
        parts = Parts::join(parts, each);
    }
    converted.then(|| Converted {
        currency: target.to_string(),
        amount,
        rest: rest.into_iter().collect(),
        fallback,
        parts,
    })
}

#[cfg(test)]
mod tests;
