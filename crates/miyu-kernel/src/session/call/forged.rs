//! 回答后面模型编的下一轮，写成回复之前截掉（施工 FT-1，`docs/blueprint/kernel/session.md`「截掉模型编的下一轮」）。认法在
//! [`crate::forged`]，这里照它截：每一块字从编的那一行的行首截到末尾，再去掉末尾的空白；截完是空的，这一块不留。思考、工具
//! 调用不动。截在哪记下来，交给 `model.called` 的 `cut`；截掉的原文哪儿都不留。

use crate::block::Block;
use crate::event::ForgedCut;
use crate::forged::{Shells, find};
use crate::session::Session;

impl Session {
    /// 照这一次请求里人这一边的外壳 `shells`，把 `numbered`（拼好的回复，每一块带着它在流里是第几块）里每一块字编的那一段
    /// 截掉，交回截了哪几处，照块的先后。没编的一个字节不动。记录的行首照这个会话的组装器认。
    pub(super) fn cut_forged(
        &self,
        shells: &Shells,
        numbered: &mut Vec<(usize, Block)>,
    ) -> Vec<ForgedCut> {
        let assembler = &self.policy.assembler;
        let record = |rest: &str| assembler.record_head(rest);
        let mut cuts = Vec::new();
        numbered.retain_mut(|(index, block)| {
            let Block::Text(text) = block else {
                return true;
            };
            let Some(forged) = find(&text.text, shells, &record) else {
                return true;
            };
            cuts.push(ForgedCut {
                block: *index as u64,
                at: forged.at as u64,
                bytes: (text.text.len() - forged.at) as u64,
                role: forged.role,
            });
            text.text.truncate(forged.at);
            let kept = text.text.trim_end().len();
            text.text.truncate(kept);
            !text.text.is_empty()
        });
        cuts
    }
}
