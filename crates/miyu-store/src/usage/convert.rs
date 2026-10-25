//! 金额换算（施工 UC-1，`docs/blueprint/models.md`「怎么走」第九条第 8 条）：一组的金额照每一笔发生那天（UTC）的汇率换成
//! 一个币种加起来；哪一天哪个币种（或者要换成的那个币种）的汇率没有，那几笔照币种各加各的留在 `rest`。一笔都没换成的没有。

use std::collections::BTreeMap;

use super::rates::{Day, Source};

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
}

/// 把 `daily`（一组里按 UTC 日子、币种分开的金额）照 `rates`（1 美元换多少）换成 `target`。
pub(crate) fn convert(
    daily: &[(Day, String, f64)],
    target: &str,
    rates: &BTreeMap<(Day, String), (f64, Source)>,
) -> Option<Converted> {
    let mut amount = 0.0;
    let mut converted = false;
    let mut fallback = false;
    let mut rest: BTreeMap<String, f64> = BTreeMap::new();
    for (day, currency, value) in daily {
        if currency == target {
            amount += value;
            converted = true;
            continue;
        }
        let from = rates.get(&(*day, currency.clone()));
        let to = rates.get(&(*day, target.to_string()));
        match (from, to) {
            (Some((from, one)), Some((to, other))) if *from > 0.0 => {
                amount += value / from * to;
                converted = true;
                fallback |= *one == Source::Fallback || *other == Source::Fallback;
            }
            _ => *rest.entry(currency.clone()).or_default() += value,
        }
    }
    converted.then(|| Converted {
        currency: target.to_string(),
        amount,
        rest: rest.into_iter().collect(),
        fallback,
    })
}

#[cfg(test)]
mod tests;
