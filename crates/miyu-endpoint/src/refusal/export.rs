//! 导出、导入会话的拒绝（施工 SE-1 上、下，`session.export`、`session.import`），和改存档的拒绝。

use super::{REFUSED, Refusal};

impl Refusal {
    /// 要写的那个文件已经有了：不覆盖。
    pub(crate) const EXPORT_EXISTS: Refusal = Refusal {
        code: REFUSED,
        reason: "export_exists",
        data: None,
    };
    /// 要写到数据根里面：不往数据根里写（同 `blob.put` 的边界）。
    pub(crate) const EXPORT_IN_DATA_ROOT: Refusal = Refusal {
        code: REFUSED,
        reason: "export_in_data_root",
        data: None,
    };
    /// 放它的目录不在，或者写不进。
    pub(crate) const EXPORT_UNWRITABLE: Refusal = Refusal {
        code: REFUSED,
        reason: "export_unwritable",
        data: None,
    };
    /// 不是认得的包：不是 ZIP、没有清单、清单读不懂。
    pub(crate) const BAD_BUNDLE: Refusal = Refusal {
        code: REFUSED,
        reason: "bad_bundle",
        data: None,
    };
    /// 包里的东西和清单对不上。
    pub(crate) const BUNDLE_MISMATCH: Refusal = Refusal {
        code: REFUSED,
        reason: "bundle_mismatch",
        data: None,
    };
    /// 包的格式比这个核心新。
    pub(crate) const BUNDLE_TOO_NEW: Refusal = Refusal {
        code: REFUSED,
        reason: "bundle_too_new",
        data: None,
    };
    /// 会话是只读存档：能翻、能搜，不能接着说。
    pub(crate) const ARCHIVED: Refusal = Refusal {
        code: REFUSED,
        reason: "archived",
        data: None,
    };
    /// 这个数据根里已经有同编号、内容不一样的会话：`data.session` 是哪一个。
    pub(crate) fn session_exists(session: &str) -> Refusal {
        Refusal::with(
            "session_exists",
            "session",
            serde_json::Value::String(session.to_string()),
        )
    }
}
