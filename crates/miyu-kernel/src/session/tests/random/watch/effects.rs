//! 看守给工具结果配的效果（施工 4-7 上起）：新建、改文件、读文件、派任务轮着来。施工 U-1 起撤销不再改回文件，效果照样记，
//! 头显示差异、改之前核对还要用。

use super::*;
use crate::event::{Effect, FileChanged};

impl Watch {
    /// 工具报的效果，不占随机数（原来那串输入不跟着错开）：照有了结果的调用数轮着来，三条里一条新建了一个文件、
    /// 一条改了一个文件，一条什么都没报。每一条还派一个任务（施工 7-1，`jobs.rs`）：
    /// 执行器交回来记下的结果三百例里只有二十来条，少派几个就凑不齐一个会话派两个。
    pub(in super::super) fn some_effects(&self) -> Vec<Effect> {
        let n = self.resulted.len();
        // 读过的文件换着来（施工 6-5）：压后重建挑候选时，尾巴里没读过的才重读。
        let read = Effect::FileRead(crate::event::FileRead {
            path: format!("/w/r{}.txt", n % 4),
            lines: Some([1, 2]),
            hash: ContentHash::of(b"read"),
            skill: None,
            display_name: Default::default(),
        });
        let changed = |before| {
            Effect::FileChanged(FileChanged {
                path: "/w/a.txt".to_string(),
                before,
                after: ContentHash::of(b"after"),
            })
        };
        let mut effects = match n % 3 {
            0 => vec![changed(None), read],
            1 => vec![changed(Some(ContentHash::of(b"before"))), read],
            _ => vec![read],
        };
        effects.push(self.some_job(n));
        effects
    }

    /// 这几轮里的工具结果一共改过几次文件：读过的、派了任务的不算。重做挑最后一轮时照它多挑改过文件的。
    pub(in super::super) fn changes_in(&self, turns: &[TurnId]) -> usize {
        self.events
            .iter()
            .filter(|event| event.turn.is_some_and(|turn| turns.contains(&turn)))
            .map(|event| match &event.body {
                Body::ToolResult(result) => result
                    .effects
                    .iter()
                    .filter(|effect| {
                        matches!(effect, Effect::FileChanged(_) | Effect::FileTrashed(_))
                    })
                    .count(),
                _ => 0,
            })
            .sum()
    }
}
