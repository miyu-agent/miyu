//! 项目的拒绝（施工 H-1，设计 `38-项目.md` 第二节第 5 条）。

use super::{REFUSED, Refusal};

impl Refusal {
    /// 没有这个项目。
    pub(crate) const UNKNOWN_PROJECT: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_project",
        data: None,
    };

    /// 参数不对，`data.field` 说是哪一格写得不对（名称、说明、颜色、工作目录、附加提示词，`unset` 列了不能清的）。
    pub(crate) fn bad_field(field: &str) -> Refusal {
        let mut data = serde_json::Map::new();
        data.insert("field".to_string(), serde_json::json!(field));
        Refusal {
            data: Some(data),
            ..Refusal::BAD_PARAMS
        }
    }
}
