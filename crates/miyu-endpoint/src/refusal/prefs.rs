//! 头自己的设置的拒绝（施工 HP-1，`docs/blueprint/protocol.md`「头自己的设置」）。

use serde_json::json;

use super::{REFUSED, Refusal};

impl Refusal {
    /// 装着这个包，但不是界面包（清单不是 `[ui]`）：只有界面包有自己的设置。
    pub(crate) const NOT_A_UI_PACKAGE: Refusal = Refusal {
        code: REFUSED,
        reason: "not_a_ui_package",
        data: None,
    };

    /// 并好的整份写成 JSON 有 `size` 个字节，超过了上限 `limit`：什么都没改。
    pub(crate) fn prefs_too_big(size: usize, limit: usize) -> Refusal {
        let mut data = serde_json::Map::new();
        data.insert("size".to_string(), json!(size));
        data.insert("limit".to_string(), json!(limit));
        Refusal {
            code: REFUSED,
            reason: "prefs_too_big",
            data: Some(data),
        }
    }
}
