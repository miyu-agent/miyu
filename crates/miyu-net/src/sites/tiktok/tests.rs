//! TikTok（施工 N-6）：认地址；页面脚本 `__UNIVERSAL_DATA_FOR_REHYDRATION__` 照录下来的 `itemStruct` 读出标题、作者、时长、
//! 封面；不存在的、没有脚本的。片段是 2026-10-24 终端实测时录的（`tests/fixtures/`），标签照真页面原样抄。

use std::path::Path;

use super::*;

/// 出厂的规矩。
fn rules() -> Tiktok {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    crate::rules::load(&resources)
        .expect("出厂的读得出")
        .sites
        .tiktok
}

fn url(text: &str) -> Url {
    Url::parse(text).expect(text)
}

/// 录下来的 `itemStruct`。
const ITEM_STRUCT: &str = include_str!("../../../tests/fixtures/tiktok-item-struct.json");

/// 视频页：`webapp.video-detail` 照给的。
fn page(detail: &str) -> String {
    format!(
        "<html><head></head><body><script id=\"__UNIVERSAL_DATA_FOR_REHYDRATION__\" type=\"application/json\">{{\"__DEFAULT_SCOPE__\":{{\"webapp.app-context\":{{}},\"webapp.video-detail\":{detail}}}}}</script><script>later()</script></body></html>"
    )
}

#[test]
fn video_pages_have_a_user_and_a_number() {
    let rules = rules();
    for yes in [
        "https://www.tiktok.com/@mochi_example/video/7440114099565972791",
        "https://www.tiktok.com/@/video/7440114099565972791?_r=1&_d=x",
        "https://tiktok.com/@mochi_example/video/7440114099565972791/",
    ] {
        assert!(is_video(&rules, &url(yes)), "{yes}");
    }
    for no in [
        "https://www.tiktok.com/@mochi_example",
        "https://www.tiktok.com/video/7440114099565972791",
        "https://www.tiktok.com/@mochi_example/video/abc",
        "https://vm.tiktok.com/ZMhvqjqjq/",
        "https://www.nottiktok.com/@mochi_example/video/1",
    ] {
        assert!(!is_video(&rules, &url(no)), "{no}");
    }
}

#[test]
fn a_video_gives_title_author_duration_and_cover() {
    let detail = format!(
        "{{\"itemInfo\":{{\"itemStruct\":{}}},\"shareMeta\":{{}},\"statusCode\":0,\"statusMsg\":\"\"}}",
        ITEM_STRUCT.trim()
    );
    let draft = item(&page(&detail)).expect("读得出");
    assert_eq!(draft.kind, Kind::Video);
    assert!(
        draft.found.title.starts_with("Example caption"),
        "{}",
        draft.found.title
    );
    assert_eq!(draft.author, "Mochi, Momo & Mimi");
    assert_eq!(draft.duration, Some(10));
    let cover = draft.found.image.expect("有封面");
    assert!(
        cover
            .as_str()
            .starts_with("https://p16-common-sign.tiktokcdn.com/"),
        "{cover}"
    );
}

#[test]
fn a_missing_or_blocked_video_has_no_card() {
    let missing = "{\"statusCode\":10204,\"statusMsg\":\"item doesn't exist\"}";
    assert!(item(&page(missing)).is_none(), "不存在的");
    let flagged = format!(
        "{{\"itemInfo\":{{\"itemStruct\":{}}},\"statusCode\":10204}}",
        ITEM_STRUCT.trim()
    );
    assert!(
        item(&page(&flagged)).is_none(),
        "statusCode 不是 0 的带着 itemStruct 也不算"
    );
    let empty = "{\"itemInfo\":{},\"statusCode\":0}";
    assert!(item(&page(empty)).is_none(), "没有 itemStruct");
    assert!(
        item("<html><body>Access Denied</body></html>").is_none(),
        "没有那段脚本"
    );
}
