//! 软件包带的供应商（施工 PV-4）：`explain` 那一层写「软件包 <编号>」，人盖住它的照常是系统配置生效。

use serde_json::json;

use super::places;
use crate::config::render::explain;
use crate::language::Language;

#[test]
fn a_package_layer_names_its_package() {
    let item = json!({"layers": [
        {"origin": {"file": "config.toml", "layer": "system", "line": 2}, "used": true, "value": "Mine"},
        {"origin": {"layer": "package", "package": "agy-demo"}, "used": false, "value": "Antigravity"},
    ]});
    let said = json!({"applies": "now", "description": "说明。", "name": "显示名"});
    let lines = explain(
        "providers.agyd.name",
        &item,
        &said,
        Language::Chinese,
        &places(),
    );
    assert_eq!(
        lines[2].paint(false),
        "  \"Antigravity\"  软件包 agy-demo\n"
    );
    let lines = explain(
        "providers.agyd.name",
        &item,
        &said,
        Language::English,
        &places(),
    );
    assert_eq!(
        lines[2].paint(false),
        "  \"Antigravity\"  package agy-demo\n"
    );
}
