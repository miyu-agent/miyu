//! 抖音（施工 N-6）：认地址；分享页的 `_ROUTER_DATA` 照录下来的作品读出标题、作者、时长、封面；删了的；兜底的 JSON-LD。
//! 片段是 2026-10-24 终端实测时录的（`tests/fixtures/`），外面的标签照真页面原样抄。

use std::path::Path;

use super::*;

/// 出厂的规矩。
fn rules() -> Douyin {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    crate::rules::load(&resources)
        .expect("出厂的读得出")
        .sites
        .douyin
}

fn url(text: &str) -> Url {
    Url::parse(text).expect(text)
}

/// 录下来的一件作品。
const ITEM: &str = include_str!("../../../tests/fixtures/douyin-item.json");

/// 录下来的网页版 `VideoObject`。
const VIDEO_OBJECT: &str = include_str!("../../../tests/fixtures/douyin-videoobject.json");

/// 分享页：照真页面的写法，`loaderData` 里一个布局、一个作品页，`item_list`、`filter_list` 照给的。
fn share_page(items: &str, filtered: &str) -> String {
    format!(
        "<html><body><script>window._ROUTER_DATA = {{\"loaderData\":{{\"video_layout\":null,\"video_(id)\\u002Fpage\":{{\"videoInfoRes\":{{\"extra\":{{}},\"filter_list\":[{filtered}],\"is_oversea\":0,\"item_list\":[{items}],\"status_code\":0}}}}}},\"errors\":null}}</script></body></html>"
    )
}

#[test]
fn works_are_videos_or_notes_by_number() {
    let rules = rules();
    for (yes, id, note) in [
        (
            "https://www.douyin.com/video/7691769993750777763",
            "7691769993750777763",
            false,
        ),
        (
            "https://www.douyin.com/video/7691769993750777763?previous_page=web_code_link",
            "7691769993750777763",
            false,
        ),
        (
            "https://www.iesdouyin.com/share/video/6918273131559881997/?region=US&mid=1",
            "6918273131559881997",
            false,
        ),
        (
            "https://www.iesdouyin.com/share/note/7000000000000000001/",
            "7000000000000000001",
            true,
        ),
        (
            "https://www.douyin.com/note/7000000000000000001",
            "7000000000000000001",
            true,
        ),
    ] {
        assert_eq!(
            id_of(&rules, &url(yes)),
            Some((id.to_string(), note)),
            "{yes}"
        );
    }
    for no in [
        "https://www.iesdouyin.com/?schema_type=20&object_id=1&is_qrcode=1",
        "https://www.douyin.com",
        "https://www.douyin.com/video/abc",
        "https://www.douyin.com/user/123",
        "https://v.douyin.com/L4FJNR3/",
        "https://www.notdouyin.com/video/123",
    ] {
        assert_eq!(id_of(&rules, &url(no)), None, "{no}");
    }
}

#[test]
fn a_shared_work_gives_title_author_duration_and_cover() {
    let Shared::Found(draft) = shared(&share_page(ITEM, "")) else {
        panic!("读得出作品");
    };
    assert_eq!(draft.kind, Kind::Video);
    assert_eq!(draft.found.title, "示例作品");
    assert_eq!(draft.author, "示例作者");
    assert_eq!(draft.duration, Some(15), "15034 毫秒四舍五入成 15 秒");
    let cover = draft.found.image.expect("有封面");
    assert!(
        cover
            .as_str()
            .starts_with("https://p11-sign.douyinpic.com/"),
        "{cover}"
    );
}

#[test]
fn a_deleted_work_is_gone_and_an_empty_page_falls_back() {
    let deleted = r#"{"aweme_id":"7126745726494821640","filter_reason":"status_self_see","notice":"抱歉，作品不见了"}"#;
    assert!(matches!(shared(&share_page("", deleted)), Shared::Gone));
    assert!(
        matches!(shared(&share_page("", "")), Shared::Missed),
        "都空的走兜底"
    );
    // 没带 cookie 的那种：`loaderData` 里没有 `videoInfoRes`。
    let bare = "<script>window._ROUTER_DATA = {\"loaderData\":{\"video_layout\":null,\"video_(id)\\u002Fpage\":{}},\"errors\":null};</script>";
    assert!(matches!(shared(bare), Shared::Missed));
    assert!(matches!(shared("<html></html>"), Shared::Missed));
}

#[test]
fn a_work_without_a_description_is_titled_by_its_author() {
    let item = r#"{"desc":"  ","author":{"nickname":"示例作者"},"video":{"duration":499}}"#;
    let Shared::Found(draft) = shared(&share_page(item, "")) else {
        panic!("有作者也算");
    };
    assert_eq!(draft.found.title, "示例作者");
    assert_eq!(draft.duration, None, "不到半秒的当没有");
    let empty = r#"{"desc":"","author":{"nickname":""}}"#;
    assert!(matches!(shared(&share_page(empty, "")), Shared::Missed));
}

#[test]
fn the_fallback_reads_the_video_object_among_the_json_ld() {
    let breadcrumb =
        r#"{"@context":"https://schema.org","@type":"BreadcrumbList","itemListElement":[]}"#;
    let page = format!(
        "<head><script data-rh=\"true\" type=\"application/ld+json\" nonce=\"oCTy\">{breadcrumb}</script><script data-rh=\"true\" type=\"application/ld+json\" nonce=\"oCTy\">{VIDEO_OBJECT}</script></head>"
    );
    let draft = video_object(&page, " - 抖音").expect("有 VideoObject");
    assert_eq!(draft.found.title, "示例作品", "去掉了「 - 抖音」");
    assert_eq!(draft.author, "示例作者");
    assert_eq!(draft.duration, Some(15));
    assert!(draft.found.image.is_some());
    assert!(
        video_object(
            &format!("<script type=\"application/ld+json\">{breadcrumb}</script>"),
            " - 抖音"
        )
        .is_none()
    );
}

#[test]
fn iso_durations_become_seconds() {
    assert_eq!(iso_seconds("PT0H0M15S"), Some(15));
    assert_eq!(iso_seconds("PT1M"), Some(60));
    assert_eq!(iso_seconds("PT1H2M3.5S"), Some(3723));
    assert_eq!(iso_seconds("P1DT1S"), Some(86_401));
    assert_eq!(iso_seconds("15"), None);
    assert_eq!(iso_seconds("PT15"), None);
    assert_eq!(iso_seconds("PT1X"), None);
}
