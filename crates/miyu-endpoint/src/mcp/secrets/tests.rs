//! 密钥变了重起谁（施工 X-32 补）：变了的名字照摘要比，新存的、删掉的、改了值的都算；服务引用了哪个照 `env`、`bearer`、`headers` 里
//! 写的 `{ secret = … }` 认，`{ env = … }`、照原样写的字不算。

use std::collections::{BTreeMap, BTreeSet};

use miyu_config::package::configured_mcp;

use super::{changed, uses};

fn names(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(ToString::to_string).collect()
}

#[test]
fn set_deleted_and_changed_values_count() {
    let before = BTreeMap::from([
        ("same".to_string(), 1),
        ("changed".to_string(), 2),
        ("deleted".to_string(), 3),
    ]);
    let now = BTreeMap::from([
        ("same".to_string(), 1),
        ("changed".to_string(), 20),
        ("added".to_string(), 4),
    ]);
    assert_eq!(
        changed(&before, &now),
        names(&["added", "changed", "deleted"])
    );
    assert!(changed(&now, &now).is_empty());
}

#[test]
fn a_server_uses_the_secrets_it_references() {
    let (servers, problems) = configured_mcp(
        "[mcp.remote]\nurl = \"https://mcp.example.com/mcp\"\nbearer = { secret = \"t\" }\nheaders = { X-Key = { secret = \"k\" } }\n\n[mcp.local]\ncommand = [\"x\"]\nenv = { A = { secret = \"a\" }, B = { env = \"t\" }, C = \"t\" }\n",
    );
    assert!(problems.is_empty(), "{problems:?}");
    let by = |name: &str| servers.iter().find(|server| server.name == name).unwrap();
    assert_eq!(uses(by("remote"), &names(&["t"])), Some("t"), "bearer");
    assert_eq!(uses(by("remote"), &names(&["k"])), Some("k"), "headers");
    assert_eq!(uses(by("local"), &names(&["a"])), Some("a"), "env");
    assert_eq!(
        uses(by("local"), &names(&["t"])),
        None,
        "环境变量的引用、照原样写的字不算"
    );
    assert_eq!(uses(by("remote"), &names(&["other"])), None);
}
