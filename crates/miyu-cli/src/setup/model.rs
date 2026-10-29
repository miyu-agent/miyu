//! 选主对话的模型（`docs/blueprint/cli/setup.md`「怎么走」第 9 条，施工 8-11）：试的那一个（推荐的）排第一，别的照列出的
//! 先后，全部列出来（施工 ST-2：项目主人 10-29 在虚拟机里接 OpenCode Go，列 20 个、说「还有 17 个」，要的是全列）；也能敲名字。
//! 不在终端里的不问，用推荐的。

use super::flow::{Flow, Tried};
use super::pick::{Row, Typed, numbered, typed};
use crate::shown::{say, write};

impl Flow<'_> {
    /// 照 `tried` 选一个，交回模型名。
    pub(super) fn pick_model(&mut self, tried: &Tried) -> Result<String, u8> {
        // 不在终端里的不问；取不到列表、自己填了模型名的不再问（施工 8-11 再补）。
        if !self.console.terminal() || tried.models.is_empty() {
            return Ok(tried.model.clone());
        }
        let language = self.plan.language;
        let ordered: Vec<&str> = std::iter::once(tried.model.as_str())
            .chain(
                tried
                    .models
                    .iter()
                    .map(String::as_str)
                    .filter(|model| *model != tried.model),
            )
            .collect();
        let rows: Vec<Row> = ordered
            .iter()
            .enumerate()
            .map(|(at, model)| {
                let suffix = if at == 0 { language.recommended() } else { "" };
                Row::usable(vec![format!("{model}{suffix}")])
            })
            .collect();
        say(self.err, language.models_heading());
        for line in numbered(&rows, None) {
            write(self.err, &line.paint(self.plan.gray));
        }
        loop {
            match typed(self.ask(language.pick_model()), ordered.len()) {
                Typed::Picked(at) => return Ok(ordered[at].to_string()),
                Typed::Empty => return Ok(tried.model.clone()),
                Typed::End => return self.not_picked(),
                Typed::Zero => say(self.err, &language.not_listed("0")),
                Typed::Other(text) if ordered.contains(&text.as_str()) => return Ok(text),
                Typed::Other(text) => say(self.err, &language.not_listed(&text)),
            }
        }
    }
}
