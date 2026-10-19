//! 限额冷却（设计 36 第二节第 4 条，项目主人 2026-10-18 定）：一家报限额，冷却照台阶走（出厂 30 分钟、1 小时、2 小时，最多
//! 6 小时）；冷却着的跳过；成功一次清零；到点自动回来。记在核心的内存里，核心重启从头算。
//!
//! 纯逻辑：时刻由调的一方给，测试不用等。

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// 各家的冷却。
#[derive(Debug, Default)]
pub(crate) struct Cooldowns {
    /// 台阶：第几次被限冷却多久，超过的照最后一级。
    ladder: Vec<Duration>,
    /// 来源的编号 → 连着被限了几次、冷却到什么时候。
    held: HashMap<String, (usize, Instant)>,
}

impl Cooldowns {
    /// 照台阶 `ladder`；空的当没有冷却。
    pub(crate) fn new(ladder: Vec<Duration>) -> Cooldowns {
        Cooldowns {
            ladder,
            held: HashMap::new(),
        }
    }

    /// `id` 在 `now` 还冷却着吗：冷却着的交回到什么时候。
    pub(crate) fn until(&self, id: &str, now: Instant) -> Option<Instant> {
        self.held
            .get(id)
            .map(|(_, until)| *until)
            .filter(|until| *until > now)
    }

    /// `id` 在 `now` 报了限额：照连着被限的次数挑台阶，交回冷却到什么时候。
    pub(crate) fn limited(&mut self, id: &str, now: Instant) -> Instant {
        let strikes = self.held.get(id).map_or(0, |(strikes, _)| *strikes);
        let wait = self
            .ladder
            .get(strikes)
            .or_else(|| self.ladder.last())
            .copied()
            .unwrap_or_default();
        let until = now + wait;
        self.held.insert(id.to_string(), (strikes + 1, until));
        until
    }

    /// 记着的几家（冷却着的、冷却过的）的编号。
    pub(crate) fn ids(&self) -> Vec<String> {
        self.held.keys().cloned().collect()
    }

    /// `id` 成功了一次：清零。
    pub(crate) fn succeeded(&mut self, id: &str) {
        self.held.remove(id);
    }
}

#[cfg(test)]
mod tests;
