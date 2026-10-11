//! 想起过什么（施工 R-4 下，`docs/blueprint/memory.md` 第八条）：这一间的记忆用到几次、最近一次，照回合库里记着的读（会话
//! 落盘时顺手写，`memory.rs` 的 `Turns`）。

use std::collections::BTreeMap;

use miyu_store::recall::Used;

use super::Keeper;

impl Keeper {
    /// 这一间每一条用到过的记忆用到几次、最近一次，键是编号（`m12` 这种）；没用到过的不在里面。回合库用不了的是空的。
    ///
    /// # Errors
    ///
    /// 回合库读不了：英文的一句为什么。
    pub fn used(&self) -> Result<BTreeMap<String, Used>, String> {
        let (turns, _) = self.memory.turns.turns(self.room());
        turns.uses().map_err(|error| error.to_string())
    }
}
