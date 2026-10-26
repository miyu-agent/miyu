//! 关掉必需的包（施工 U23 末上）：`config.set` 那一道只拒写成关着的必需的包，别的照收。

use std::collections::BTreeSet;

use miyu_config::Value;
use miyu_config::problem::Code;

use super::problem;

#[test]
fn only_switching_a_required_package_off_is_refused() {
    let required: BTreeSet<String> = ["basesystem".to_string()].into();
    assert_eq!(
        problem("packages.basesystem.off", &Value::Bool(true), &required),
        Some((Code::NotAnOption, "true".to_string()))
    );
    assert_eq!(
        problem("packages.basesystem.off", &Value::Bool(false), &required),
        None,
        "写成开着的照收"
    );
    assert_eq!(
        problem("packages.kit.off", &Value::Bool(true), &required),
        None,
        "不是必需的照收"
    );
    assert_eq!(
        problem("features.skill-x", &Value::Bool(true), &required),
        None,
        "别的键不管"
    );
}
