//! 问了一遍退不退以后，没退下的照原因各几个（施工 MEM-3，`docs/blueprint/protocol.md`「会话表」第 10 条「记什么」）：内存紧
//! 时卸不动的，日志里看得出是被什么挡着（有回合在跑、有人订阅、记忆抽取在路上……）。

use std::fmt;

use miyu_session::Hold;

/// 没退下的照原因各几个：一样一格，照 [`Hold::ALL`] 的先后。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Held([usize; Hold::ALL.len()]);

impl Held {
    /// 又一个因为 `hold` 没退下。
    pub(crate) fn add(&mut self, hold: Hold) {
        if let Some(slot) = Hold::ALL.iter().position(|one| *one == hold) {
            self.0[slot] += 1;
        }
    }

    /// 因为 `hold` 没退下的几个。
    #[cfg(test)]
    pub(crate) fn of(&self, hold: Hold) -> usize {
        Hold::ALL
            .iter()
            .position(|one| *one == hold)
            .map_or(0, |slot| self.0[slot])
    }
}

/// 记日志的样子：有的几样照先后 `<原因>=<几个>`、空格隔开，例如 `turn=3 memory=12`；一个都没有的是 `none`。
impl fmt::Display for Held {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for (hold, count) in Hold::ALL.iter().zip(self.0) {
            if count == 0 {
                continue;
            }
            if !first {
                f.write_str(" ")?;
            }
            write!(f, "{}={count}", hold.name())?;
            first = false;
        }
        if first {
            f.write_str("none")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_in_order_and_none_when_empty() {
        let mut held = Held::default();
        assert_eq!(held.to_string(), "none");
        held.add(Hold::Memory);
        held.add(Hold::Turn);
        held.add(Hold::Memory);
        assert_eq!(held.to_string(), "turn=1 memory=2");
        assert_eq!(held.of(Hold::Memory), 2);
        assert_eq!(held.of(Hold::Slow), 0);
    }
}
