use std::path::PathBuf;

use miyu_config::package::read;
use miyu_store::packages::{Found, Issue, Layer};

use super::*;

/// 一个读成了的包，`depends` 照写的；`broken` 的读坏了。
fn package(id: &str, depends: &[&str], broken: bool) -> Found {
    let listed = depends
        .iter()
        .map(|id| format!("\"{id}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let text = format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"{id}\" }}\n\n[builtin]\n\n[depends]\npackages = [{listed}]\n"
    );
    let read = match broken {
        true => Err(Issue::Unreadable(std::io::Error::other("bad"))),
        false => Ok(read(&text).expect("清单合写法")),
    };
    Found {
        id: id.to_string(),
        layer: Layer::Home,
        path: PathBuf::from(format!("{id}/package.toml")),
        read,
    }
}

#[test]
fn only_read_packages_count_as_installed() {
    let qq = package("qq-admin", &["onebot", "embed"], false);
    let onebot = package("onebot", &[], false);
    let broken = package("embed", &[], true);
    let manifest = qq.read.as_ref().expect("读成了");
    assert_eq!(
        missing(&[&onebot, &broken], manifest),
        ["embed"],
        "读坏了的不算装着"
    );
    assert_eq!(missing(&[], manifest), ["onebot", "embed"], "照写的先后");
}

#[test]
fn dependents_are_the_other_read_packages_naming_it() {
    let installed = [
        package("zz-admin", &["onebot"], false),
        package("aa-admin", &["onebot"], false),
        package("onebot", &["onebot"], false),
        package("other", &[], false),
    ];
    assert_eq!(dependents(&installed, "onebot"), ["aa-admin", "zz-admin"]);
    assert!(dependents(&installed, "other").is_empty());
}
