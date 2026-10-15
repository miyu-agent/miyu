//! 看守查读回日志的时候（施工 6-9）：新来的命令一律拒绝，原因码 `restoring`，不开新的一轮。施工 U-1 起撤销不再改回文件，
//! `restoring` 只剩这一种。

use super::*;

impl Watch {
    /// 新来的命令是不是撞上了读回日志：撞上的，别的看守不再照自己的规矩判。
    pub(super) fn reading_refuses(&self, input: &Input) -> bool {
        self.undo.reading.is_some()
            && matches!(input, Input::Command(received) if self.fresh(&received.id))
    }

    /// 送进去以后：撞上了的，该拒绝，原因码 `restoring`。
    pub(super) fn after_reading(&mut self, actions: &[Action], refused: bool) {
        if !refused {
            return;
        }
        let seed = self.seed;
        self.seen_paths.insert("读回日志时来的命令被拒");
        assert!(
            matches!(
                actions,
                [Action::Reply {
                    outcome: Outcome::Rejected {
                        reason: Reason::Restoring,
                        ..
                    },
                    ..
                }]
            ),
            "种子 {seed}：读回日志的时候来的命令应该拒绝，原因码 restoring：{actions:?}"
        );
    }
}
