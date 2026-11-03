//! 导出会话的三种拒绝（施工 SE-1 上，`session.export`）。

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
}
