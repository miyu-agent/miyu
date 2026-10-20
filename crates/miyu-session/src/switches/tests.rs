//! 全局开关的那一项（施工 X-27）：只认三类功能编号，没写的算开。

use super::*;

#[test]
fn only_the_three_kinds_are_read() {
    let mut values = Values::defaults(&[]);
    values.set("features.skill-weather", Value::Bool(false));
    values.set("features.mcp-github", Value::Bool(true));
    values.set("features.memory", Value::Bool(false));
    values.set("loading.tools.weather", Value::Bool(false));
    let switches = of(&values);
    assert_eq!(
        switches.into_iter().collect::<Vec<_>>(),
        [
            ("mcp-github".to_string(), true),
            ("skill-weather".to_string(), false)
        ]
    );
    let switches = of(&values);
    assert!(!on(&switches, "skill-weather"));
    assert!(on(&switches, "script-echo"), "没写的算开");
}
