use std::path::PathBuf;

use miyu_config::package::read;
use miyu_store::packages::{Found, Issue, Layer};

use super::*;

/// 编号 `id`、清单 `text` 的一个包；`text` 是空的读坏了。
fn package(id: &str, text: &str) -> Found {
    let read = match text.is_empty() {
        true => Err(Issue::Unreadable(std::io::Error::other("bad"))),
        false => Ok(read(text).expect("清单合写法")),
    };
    Found {
        id: id.to_string(),
        layer: Layer::Shipped,
        path: PathBuf::from(format!("{id}/package.toml")),
        read,
    }
}

/// 功能在哪个包里（施工 PN-2）：写明的功能照写的，没写的照包编号算一个；读坏了的不算。
#[test]
fn homes_map_each_feature_to_its_package() {
    let written = "[package]\nprotocol = [1, 1]\nname = { en = \"M\" }\n\n[features.memory]\nname = { en = \"M\" }\n\n[builtin]\n";
    let plain = "[package]\nprotocol = [1, 1]\nname = { en = \"G\" }\n\n[builtin]\n";
    let homes = homes(&[
        package("miyu-base-memory", written),
        package("goal", plain),
        package("broken", ""),
    ]);
    assert_eq!(
        homes.into_iter().collect::<Vec<_>>(),
        [
            ("goal".to_string(), "goal".to_string()),
            ("memory".to_string(), "miyu-base-memory".to_string()),
        ]
    );
}
