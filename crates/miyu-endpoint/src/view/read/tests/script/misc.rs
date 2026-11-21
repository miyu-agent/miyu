//! 不在回合里的几样（施工 FX-16 从 `script.rs` 挪出来）：改标题、斜杠命令、不认识的种类，换人格、预设和换上快照（人换的先记着，
//! 内核换上快照，带 `policy` 的那一条，才算数；翻页照切点前的这几条学）。

use serde_json::json;

use super::{Dice, KERNEL, PERSON, POLICY, Script, Shape};

impl Script {
    /// 改标题、换人格和预设、换上快照、斜杠命令、不认识的种类。
    pub(super) fn misc(&mut self, dice: &mut Dice, shape: Shape) {
        match dice.below(5) {
            0 => self.push(
                "session.meta_changed",
                None,
                PERSON,
                &json!({"title": "标题"}),
                shape,
            ),
            1 => self.push(
                "command.ran",
                None,
                PERSON,
                &json!({"text": "/clear", "command": "clear"}),
                shape,
            ),
            3 | 4 => self.policy(dice, shape),
            _ => self.push(
                "ext.memory.recalled",
                self.open,
                KERNEL,
                &json!({"hits": [{"id": "m1"}]}),
                shape,
            ),
        };
    }

    /// 人换人格、预设，或者内核换上快照（权限有时一起换）。交回它的序号。
    fn policy(&mut self, dice: &mut Dice, shape: Shape) -> u64 {
        if dice.odds(2) {
            let persona = *dice.pick(&["persona-2", "", "persona-1"]);
            let body = json!({"persona": persona, "preset": "chat"});
            return self.push("session.policy_changed", None, PERSON, &body, shape);
        }
        let permission = json!({"level": "full", "read_only": dice.odds(2)});
        let body = json!({"policy": POLICY, "permission": permission});
        self.push("session.policy_changed", None, KERNEL, &body, shape)
    }
}
