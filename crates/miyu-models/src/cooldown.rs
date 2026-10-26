//! 冷却（`docs/blueprint/models.md`「怎么走」第五条第 2 条，施工 8-9）：一个端点出错以后多久不再挑它。
//!
//! - 四类记冷却：限速 `rate_limited`、可重试 `retryable`、认证失败 `auth`、额度用完 `quota`（施工 Q-4 中从认证失败里分出来：
//!   供应商说了什么时候重置的照说的，最多一天；没说的照 `[models.cooldown.quota]`，默认 30 分钟）。别的分类（超长、
//!   内容策略、请求本身有错）换了端点也一样，不记（`15-模型与供应商.md` 第五节那张表）。
//! - 单位：这一家的这个模型，认证失败也一样（施工 8-25：一家一个 key，冷却只管模型；项目主人 2026-10-19 报的：中转站把一个
//!   上游的 401 透传过来，原来停整家会连累别的上游，整家的那一种去掉了）。只有经池调用的才记（会话那一层判，单个模型出错不记）。
//! - 多久：这个单位连着失败的第 n 次，冷却 `min(base × 2^(n−1), max)`；供应商说了要等多久、比它长的，用供应商说的，也不超过
//!   `max`。`base`、`max` 照分类取 `[models.cooldown]`（[`Rules`]）。
//! - 到点了就能用，失败次数不清零：再失败照 n+1 算。真成功一次才清零（第四条第 5 条）。旧版
//!   实测：固定的冷却让一直挂着的端点每两分钟被重新信任一次。
//! - 纯逻辑：时刻由调用的一方交进来。冷却表核心一份、只在内存里，放在会话那一层的 `ModelData`（重启从头来）。

use std::collections::BTreeMap;
use std::time::Duration;

use miyu_config::Values;
use miyu_kernel::event::ErrorClass;
use miyu_kernel::time::Timestamp;

use crate::settings::{AuthCooldown, QuotaCooldown, RateLimitedCooldown, RetryableCooldown};

/// 一类错冷却多久：第一次多久、最多多久。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    /// 第一次冷却多久。
    pub base: Duration,
    /// 最多冷却多久。`base` 比它大的照它。
    pub max: Duration,
}

/// `[models.cooldown]` 三类的规矩（施工 8-9）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rules {
    /// 限速。
    pub rate_limited: Rule,
    /// 可重试：连不上、5xx。
    pub retryable: Rule,
    /// 认证失败：停整家供应商。
    pub auth: Rule,
    /// 额度、余额用完（施工 Q-4 中）：供应商没说什么时候重置时照它；说了的照说的，最多 [`QUOTA_SAID_MOST`]。
    pub quota: Rule,
}

/// 额度用完、供应商说了什么时候重置的，最多照它冷却多久：一天（施工 Q-4 中，主会话定的封顶）。
pub const QUOTA_SAID_MOST: Duration = Duration::from_secs(86_400);

impl Rules {
    /// 照配置的最终值：没写的照默认。
    pub fn from_values(values: &Values) -> Rules {
        let rate_limited = RateLimitedCooldown::from(values);
        let retryable = RetryableCooldown::from(values);
        let auth = AuthCooldown::from(values);
        let quota = QuotaCooldown::from(values);
        Rules {
            rate_limited: Rule {
                base: rate_limited.base,
                max: rate_limited.max,
            },
            retryable: Rule {
                base: retryable.base,
                max: retryable.max,
            },
            auth: Rule {
                base: auth.base,
                max: auth.max,
            },
            quota: Rule {
                base: quota.base,
                max: quota.max,
            },
        }
    }

    /// 分类 `class` 的规矩；不记冷却的分类没有。
    fn of(&self, class: &ErrorClass) -> Option<Rule> {
        match class {
            ErrorClass::RateLimited => Some(self.rate_limited),
            ErrorClass::Retryable => Some(self.retryable),
            ErrorClass::Auth => Some(self.auth),
            ErrorClass::Quota => Some(self.quota),
            _ => None,
        }
    }
}

impl Default for Rules {
    /// 全照配置项的默认值：数只写在配置项的声明里一处。
    fn default() -> Rules {
        Rules::from_values(&Values::default())
    }
}

/// 一个候选（`models.md`「怎么走」第四条）：哪一家、哪个模型。冷却照它查、照它记。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Candidate {
    /// 供应商的编号。
    pub provider: String,
    /// 模型名。
    pub model: String,
}

impl Candidate {
    /// `provider` 的 `model`。
    pub fn new(provider: &str, model: &str) -> Candidate {
        Candidate {
            provider: provider.to_string(),
            model: model.to_string(),
        }
    }
}

/// 一个单位这时在冷却：到什么时候、为什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cooling {
    /// 最早什么时候能用。
    pub until: Timestamp,
    /// 为什么：那一次出错的分类。
    pub class: ErrorClass,
}

/// 记了一次失败：冷却到什么时候、冷却多久、这个单位连着第几次。运行日志 `endpoint cooling` 那一行照它写。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recorded {
    /// 冷却到什么时候。
    pub until: Timestamp,
    /// 冷却多少毫秒。
    pub for_ms: u64,
    /// 连着第几次，从 1 数。
    pub failures: u32,
}

/// 一个单位记着的：连着失败了几次、冷却到什么时候、最近一次为什么。
#[derive(Debug, Clone)]
struct Entry {
    failures: u32,
    until: Timestamp,
    class: ErrorClass,
}

/// 冷却表：核心一份，只在内存里。
#[derive(Debug, Default)]
pub struct Cooldowns {
    units: BTreeMap<Candidate, Entry>,
}

impl Cooldowns {
    /// 候选 `candidate` 在 `now` 这一刻出错了，分类 `class`，供应商说了要等 `said_ms` 毫秒（没说的没有）：照 `rules` 记冷却。
    /// 不记冷却的分类什么都不记，交回没有。
    pub fn fail(
        &mut self,
        candidate: &Candidate,
        class: &ErrorClass,
        said_ms: Option<u64>,
        rules: &Rules,
        now: Timestamp,
    ) -> Option<Recorded> {
        let rule = rules.of(class)?;
        let failures = self
            .units
            .get(candidate)
            .map_or(1, |entry| entry.failures.saturating_add(1));
        let max = millis(rule.max);
        let doubled = 1u64
            .checked_shl(failures - 1)
            .map_or(u64::MAX, |times| millis(rule.base).saturating_mul(times));
        let for_ms = match (class, said_ms) {
            // 额度用完、说了什么时候重置的（施工 Q-4 中）：照说的，短的也照，最多一天。
            (ErrorClass::Quota, Some(said)) => said.min(millis(QUOTA_SAID_MOST)),
            _ => doubled.max(said_ms.unwrap_or(0)).min(max),
        };
        let until = later(now, for_ms);
        self.units.insert(
            candidate.clone(),
            Entry {
                failures,
                until,
                class: class.clone(),
            },
        );
        Some(Recorded {
            until,
            for_ms,
            failures,
        })
    }

    /// 候选 `candidate` 成了：失败次数清零，冷却也没了。
    pub fn succeed(&mut self, candidate: &Candidate) {
        self.units.remove(candidate);
    }

    /// 候选 `candidate` 在 `now` 这一刻是不是半开（施工 Q-4 下）：冷却到期了、失败次数还没清零（成功过一次才清零）。是的话交回
    /// 它冷却到的那一刻：照它挑最早恢复的那一个。
    pub fn recovered(&self, candidate: &Candidate, now: Timestamp) -> Option<Timestamp> {
        self.units
            .get(candidate)
            .filter(|entry| entry.until <= now)
            .map(|entry| entry.until)
    }

    /// 候选 `candidate` 在 `now` 这一刻在不在冷却：在的话到什么时候、为什么。
    pub fn cooling(&self, candidate: &Candidate, now: Timestamp) -> Option<Cooling> {
        self.units
            .get(candidate)
            .filter(|entry| entry.until > now)
            .map(|entry| Cooling {
                until: entry.until,
                class: entry.class.clone(),
            })
    }
}

/// 一段时长的毫秒数，大得放不下的照最大的算。
fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// `now` 再过 `ms` 毫秒；超出能写的范围的就是 `now`：冷却的上限最多一天，实际碰不到。
fn later(now: Timestamp, ms: u64) -> Timestamp {
    i64::try_from(ms)
        .ok()
        .and_then(|ms| now.unix_millis().checked_add(ms))
        .and_then(Timestamp::from_unix_millis)
        .unwrap_or(now)
}

#[cfg(test)]
mod tests;
