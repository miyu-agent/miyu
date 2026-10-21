//! 管理员放行出沙盒的工具（施工 X-36）：照名字和来处认，来处对不上的、只写了名字没写对键的不算。

use std::sync::Arc;

use miyu_kernel::tool::Access;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Shelf, Tool};

use super::*;

fn values(pairs: &[(&str, &str)]) -> Values {
    let mut values = Values::defaults(&[]);
    for (key, set) in pairs {
        values.set(key, Value::Text(set.to_string().into()));
    }
    values
}

/// 架子上：`~home` 的 `light`、链接成的 `linked`，`basesystem` 的 `shell`、`smart-home` 的 `lamp`。
fn shelf() -> Shelf {
    let fake = |name: &str| Fake::new(name, Access::Execute, Act::Echo) as Arc<dyn Tool>;
    let linked = Fake::linked("linked", Access::Execute, Act::Echo) as Arc<dyn Tool>;
    Shelf::new(
        Catalog::in_packages([
            ("~home", vec![fake("light"), linked]),
            ("basesystem", vec![fake("shell")]),
            ("smart-home", vec![fake("lamp")]),
        ])
        .expect("合写法"),
    )
}

#[test]
fn a_grant_counts_only_when_the_source_matches() {
    let table = Unsandboxed::of(&values(&[
        ("sandbox.tools.light", "~home"),
        ("sandbox.tools.shell", "basesystem"),
        ("sandbox.tools.lamp", "other-home"),
        ("sandbox.tools.gone", "~home"),
    ]));
    let shelf = shelf();
    assert!(table.granted("light", &shelf));
    assert!(table.granted("shell", &shelf), "shell 也认");
    assert!(!table.granted("lamp", &shelf), "来处对不上的不算");
    assert!(!table.granted("gone", &shelf), "目录里没有的不算");
    assert!(!table.granted("unlisted", &shelf), "没写的不算");
    assert_eq!(
        table.entries().collect::<Vec<_>>(),
        [
            ("gone", "~home"),
            ("lamp", "other-home"),
            ("light", "~home"),
            ("shell", "basesystem"),
        ]
    );
}

/// 文件是链接的不算放行（施工 X-37）：链接能指到她改得了的地方。
#[test]
fn a_linked_tool_is_not_granted() {
    let table = Unsandboxed::of(&values(&[("sandbox.tools.linked", "~home")]));
    assert!(!table.granted("linked", &shelf()));
}

#[test]
fn only_sandbox_tools_keys_count() {
    let table = Unsandboxed::of(&values(&[
        ("sandbox.light", "~home"),
        ("clicall.tools.light", "~home"),
    ]));
    assert_eq!(table, Unsandboxed::default());
}

/// 工具名那一段带引号写的也认得（同 `clicall.tools.<name>`）。
#[test]
fn a_quoted_tool_name_is_read() {
    let table = Unsandboxed::of(&values(&[("sandbox.tools.\"mcp__home__Lamp\"", "~home")]));
    assert_eq!(
        table.entries().collect::<Vec<_>>(),
        [("mcp__home__Lamp", "~home")]
    );
}

/// 只认系统配置：写在个人设置里的读的时候就报出来，不进这一轮的值。
#[test]
fn personal_settings_cannot_grant() {
    let source = "[sandbox.tools]\nlight = \"~home\"\n";
    let personal = miyu_config::parse::parse(ITEMS, Layer::Personal, source).expect("写法对");
    assert!(!personal.problems.is_empty(), "个人设置里不收");
    let system = miyu_config::parse::parse(ITEMS, Layer::System, source).expect("写法对");
    assert!(system.problems.is_empty(), "{:?}", system.problems);
}
