//! 共用那份（施工 PL-1 修）：`<编号>@system` 照原编号对档案、对目录。

use super::*;

/// 共用那份（施工 PL-1 修）：`<编号>@system` 照原编号对档案、对目录，配置照它自己的那份读。网页 2026-10-26 实测，`deepseek@system`
/// 拿带后缀的编号去对，没有地址、`needs base_url`。
#[test]
fn a_shared_copy_is_matched_by_its_original_id() {
    let values = crate::test_support::resolved_two(
        "[providers.deepseek]\nkey = { env = \"SHARED_KEY\" }\n\n[providers.opencodego]\nlocal = false\n",
        "[providers.deepseek]\nkey = { env = \"MINE\" }\n\n[providers.opencodego]\nlocal = false\n",
    )
    .values();
    let held = held(true);
    let deepseek = provider(&values, &held.knowledge(), "deepseek@system").expect("档案推得出");
    assert_eq!(
        deepseek.base_url,
        Address::Literal("https://api.deepseek.com".to_string())
    );
    assert_eq!(deepseek.key, Some(KeyRef::Env("SHARED_KEY".to_string())));
    assert_eq!(deepseek.catalog, "deepseek");
    let go = provider(&values, &held.knowledge(), "opencodego@system").expect("目录推得出");
    assert_eq!(
        go.base_url,
        Address::Literal("https://opencode.ai/zen/go/v1".to_string())
    );
    assert_eq!(
        go.recognized.map(|recognized| recognized.provider),
        Some("opencode-go".to_string())
    );
}
