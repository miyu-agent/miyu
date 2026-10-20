//! 界面块的两种拒绝（施工 X-15 中，`docs/blueprint/widgets.md`「程序发给核心」）。

use super::Refusal;

impl Refusal {
    /// `widget.set`、`widget.clear` 的块不在这个包的清单里：`data.widget` 是哪一块。
    pub(crate) fn unknown_widget(widget: &str) -> Refusal {
        Refusal::with(
            "unknown_widget",
            "widget",
            serde_json::Value::String(widget.to_string()),
        )
    }
    /// 内容写法不对：`data.path` 是错在哪，`data.problem` 是哪一种。
    pub(crate) fn bad_widget_content(bad: &miyu_config::widget::Bad) -> Refusal {
        let mut refusal = Refusal::with(
            "bad_widget_content",
            "path",
            serde_json::Value::String(bad.path.clone()),
        );
        if let Some(data) = &mut refusal.data {
            data.insert("problem".to_string(), bad.problem.as_str().into());
        }
        refusal
    }
}
