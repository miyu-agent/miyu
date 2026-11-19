//! `miyu pkg search` 一行后面接的标记（施工 PL-2、WD-2）。

use serde_json::json;

use super::shown;
use crate::language::Language;

/// 不支持本机系统的包接上标记：中文和核心的拒绝原话、设置项说明、两个界面是同一句「不支持当前系统」。
#[test]
fn a_package_for_another_system_is_marked_in_the_language() {
    let rows = [json!({
        "package": "win-only",
        "name": "Windows only",
        "summary": "",
        "official": true,
        "unsupported": true
    })];
    assert_eq!(
        shown(&rows, &Language::Chinese),
        ["win-only  Windows only（不支持当前系统）"]
    );
    assert_eq!(
        shown(&rows, &Language::English),
        ["win-only  Windows only (not supported on this system)"]
    );
}
