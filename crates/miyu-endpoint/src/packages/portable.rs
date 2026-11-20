//! 包里的工具说明有不是各家都收的参数写法（施工 FX-10，`miyu_tool::portable`）：装包的回应、`miyu check` 各报一条警告
//! `tool_not_portable`，包照装、照用。打包时（`miyu pkg pack`）是错误，在命令行那一头拦。

use std::path::Path;

use serde_json::{Value, json};

use miyu_store::human::Human;
use miyu_tool::portable::{Found, scan};

/// 代码。
pub(crate) const CODE: &str = "tool_not_portable";

/// 包文件夹 `dir` 里每一份不是各家都收的工具说明一条警告（[`said`]）。读盘：`miyu check` 本来就在阻塞线程里查文件；装包那一头
/// 先在阻塞线程里 [`scan`]，再交给 [`said`]。
pub(crate) fn warnings(words: &Human, dir: &Path) -> Vec<Value> {
    said(words, scan(dir))
}

/// 查到的每一份一条 `{"code", "level", "message"}`，照路径排；没有的是空的。说法的字段 `detail` 是「那一份 那几处」。
pub(crate) fn said(words: &Human, found: Vec<Found>) -> Vec<Value> {
    found
        .into_iter()
        .map(|found| {
            let detail = format!("{} {}", found.file, shown(&found.at));
            let message = super::sentence(words, CODE, &detail);
            json!({"code": CODE, "level": "warning", "message": message.unwrap_or(detail)})
        })
        .collect()
}

/// 参数格式里的几处，用 `, ` 连起来；根写成 `/`。
fn shown(at: &[String]) -> String {
    at.iter()
        .map(|at| if at.is_empty() { "/" } else { at.as_str() })
        .collect::<Vec<_>>()
        .join(", ")
}
