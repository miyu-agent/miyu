//! 认出来的供应商照认出的那一家的档案（施工 DM-1）。从 `tests.rs` 分出来，照它的几样（`go_held`、`values`）。

use super::super::*;
use super::{go_held, values};

/// 编号写法不同、照地址认出来的，照认出的那一家的档案带头（施工 DM-1：分享出去的打包版里 `opencodego` 少了
/// `x-opencode-session`，opencode 回 400）；手写了 `catalog` 的照写的，编号就有档案的照编号的，都不改。
#[test]
fn a_recognized_provider_takes_the_profile_of_what_it_was_recognized_as() {
    let held = go_held(true);
    let expected = [(
        "x-opencode-session".to_string(),
        "ses_4892557fffff09df36791e54c5".to_string(),
    )];
    let values = values(
        "[providers.opencodego]\nlocal = false\n\n\
         [providers.relay]\nlocal = false\nbase_url = \"https://opencode.ai/zen/go/v1\"\n\n\
         [providers.pinned]\nlocal = false\ncatalog = \"deepseek\"\nbase_url = \"https://opencode.ai/zen/go/v1\"\n",
    );
    for id in ["opencodego", "relay"] {
        let found = provider(&values, &held.knowledge(), id).expect("目录推得出");
        assert_eq!(found.headers("ses-1"), expected, "{id} 认成 opencode-go");
        assert_eq!(
            found.catalog, "opencode-go",
            "{id} 查的是认出的那一家的档案"
        );
    }
    let pinned = provider(&values, &held.knowledge(), "pinned").expect("目录推得出");
    assert!(pinned.headers("ses-1").is_empty(), "手写的 catalog 照写的");
    assert_eq!(pinned.catalog, "deepseek");
}
