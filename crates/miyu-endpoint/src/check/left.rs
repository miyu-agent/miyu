//! `check` 说已经卸掉的包留在配置里的设置项（施工 PK-5补，从 `check.rs` 分出来，行数）：读的时候这几个键不报不认识
//! （`config/left.rs`），这里一个包一条警告 `settings_left`。用警告：头把不是警告的都算错误。

use serde_json::{Value, json};

use miyu_config::Words;
use miyu_store::human::Human;

/// 拿掉的留下的键 `leftover`（键和包）照包数一数，一个包一条：有几项、是谁留下的，重装照原样生效，不要了用 `--purge` 删。
/// `file`、`kind` 由调用的一方补。
pub(super) fn settings_left(words: &Human, leftover: &[(String, String)]) -> Vec<Value> {
    crate::config::left::counted(leftover)
        .into_iter()
        .map(|(id, count)| {
            let key = match count {
                1 => "config/settings-left/one",
                _ => "config/settings-left",
            };
            let message = Words::sentence(
                words,
                key,
                &[("count", &count.to_string()), ("package", &id)],
            );
            json!({"code": "settings_left", "level": "warning", "message": message.unwrap_or_default()})
        })
        .collect()
}
