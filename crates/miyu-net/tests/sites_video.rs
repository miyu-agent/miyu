//! 抖音、TikTok 的视频卡片（施工 N-6），全在本机的假服务器上：真的主机名经测试的口子解析到回环。抖音一个作品要的几样
//! （注册 `ttwid`、分享页、兜底的网页版）协议、端口都照认出来的那一页，所以一台假服务器就够。页面照 2026-10-24 终端录下来
//! 的片段（`fixtures/`），外面的标签照真页面原样抄。

use crate::support::{Reply, Site, Store, card, local_with, miss, resources};
use miyu_net::{Kind, LinkPreview, Why};

/// 测试里当真的那几个主机名。
const NAMES: &[&str] = &[
    "www.douyin.com",
    "www.iesdouyin.com",
    "v.douyin.com",
    "ttwid.bytedance.com",
    "www.tiktok.com",
    "vm.tiktok.com",
];

/// 录下来的一件抖音作品、网页版的 `VideoObject`、TikTok 的 `itemStruct`。
const ITEM: &str = include_str!("fixtures/douyin-item.json");
const VIDEO_OBJECT: &str = include_str!("fixtures/douyin-videoobject.json");
const ITEM_STRUCT: &str = include_str!("fixtures/tiktok-item-struct.json");

/// 录下来的那件作品的编号。
const ID: &str = "7691769993750777763";

fn previewer(store: &Store) -> LinkPreview {
    LinkPreview::new(&resources(), store.blobs.clone()).testing(local_with(NAMES))
}

/// 注册 `ttwid` 的回应：正文照真的，要的是 `Set-Cookie`。
fn registered() -> Reply {
    Reply::bytes(
        "application/json",
        br#"{"status_code":0,"message":"union register success"}"#,
    )
    .with_header(
        "Set-Cookie",
        "ttwid=1%7Cabc; Path=/; Domain=bytedance.com; Max-Age=31536000; HttpOnly; Secure; SameSite=None",
    )
}

/// 分享页：`item_list`、`filter_list` 照给的。
fn share_page(items: &str, filtered: &str) -> Reply {
    Reply::html(&format!(
        "<html><body><script>window._ROUTER_DATA = {{\"loaderData\":{{\"video_layout\":null,\"video_(id)\\u002Fpage\":{{\"videoInfoRes\":{{\"extra\":{{}},\"filter_list\":[{filtered}],\"is_oversea\":0,\"item_list\":[{items}],\"status_code\":0}}}}}},\"errors\":null}}</script></body></html>"
    ))
}

/// 网页版：有 `VideoObject` 的，或者只有个标题的空壳。
fn web_page(with_video: bool) -> Reply {
    let ld = if with_video {
        format!(
            "<script data-rh=\"true\" type=\"application/ld+json\" nonce=\"oCTy\">{VIDEO_OBJECT}</script>"
        )
    } else {
        String::new()
    };
    Reply::html(&format!(
        "<html><head><title>抖音-记录美好生活</title>{ld}</head><body></body></html>"
    ))
}

/// TikTok 的视频页：`webapp.video-detail` 照给的。
fn tiktok_page(detail: &str) -> Reply {
    Reply::html(&format!(
        "<html><head><title>TikTok - Make Your Day</title></head><body><script id=\"__UNIVERSAL_DATA_FOR_REHYDRATION__\" type=\"application/json\">{{\"__DEFAULT_SCOPE__\":{{\"webapp.video-detail\":{detail}}}}}</script></body></html>"
    ))
}

#[tokio::test]
async fn a_douyin_video_is_read_from_the_share_page_with_an_anonymous_ttwid() {
    let site = Site::start(vec![
        ("/ttwid/union/register/".to_string(), registered()),
        (format!("/share/video/{ID}/"), share_page(ITEM, "")),
        ("/share/video/1/".to_string(), share_page(ITEM, "")),
    ])
    .await;
    let port = site.port;
    let store = Store::new();
    let links = previewer(&store);
    let found = card(
        &links,
        &format!("http://www.iesdouyin.com:{port}/share/video/{ID}/?region=US&mid=1"),
    )
    .await;
    assert_eq!(found.kind, Kind::Video);
    assert_eq!(found.title, "2026年国庆节");
    assert_eq!(found.author.as_deref(), Some("笑悟人生"));
    assert_eq!(found.duration, Some(15));
    assert_eq!(found.site, "抖音");
    assert_eq!(
        found.url,
        format!("http://www.douyin.com:{port}/video/{ID}"),
        "卡片写网页版的地址"
    );
    let seen = site.seen();
    let register: Vec<_> = seen
        .iter()
        .filter(|s| s.target == "/ttwid/union/register/")
        .collect();
    assert_eq!(register.len(), 1);
    assert_eq!(register[0].method, "POST");
    assert!(
        register[0].host.starts_with("ttwid.bytedance.com"),
        "{:?}",
        register[0]
    );
    let share = seen
        .iter()
        .find(|s| s.target == format!("/share/video/{ID}/"))
        .expect("读了分享页");
    assert_eq!(share.cookie, "ttwid=1%7Cabc", "只带 ttwid，原样");
    assert!(share.user_agent.contains("iPhone"), "{}", share.user_agent);
    // 再要一件：记着的 ttwid 照用，不再注册。
    card(&links, &format!("http://www.douyin.com:{port}/video/1")).await;
    assert_eq!(site.hits("/ttwid/union/register/"), 1, "记着一天");
}

#[tokio::test]
async fn without_the_share_page_douyin_falls_back_to_the_web_page() {
    // 注册不成（这台没有它，回 404）：直接走兜底。
    let site = Site::start(vec![(format!("/video/{ID}"), web_page(true))]).await;
    let port = site.port;
    let store = Store::new();
    let found = card(
        &previewer(&store),
        &format!("http://www.douyin.com:{port}/video/{ID}"),
    )
    .await;
    assert_eq!(found.kind, Kind::Video);
    assert_eq!(found.title, "2026年国庆节", "去掉了「 - 抖音」");
    assert_eq!(found.author.as_deref(), Some("笑悟人生"));
    assert_eq!(found.duration, Some(15));
    let web = site
        .seen()
        .into_iter()
        .find(|s| s.target == format!("/video/{ID}"))
        .expect("读了网页版");
    assert!(web.user_agent.contains("bingbot"), "{}", web.user_agent);

    // 分享页里没有作品数据（都空）的也走兜底。
    let site = Site::start(vec![
        ("/ttwid/union/register/".to_string(), registered()),
        (format!("/share/video/{ID}/"), share_page("", "")),
        (format!("/video/{ID}"), web_page(true)),
    ])
    .await;
    let found = card(
        &previewer(&Store::new()),
        &format!("http://www.douyin.com:{}/video/{ID}", site.port),
    )
    .await;
    assert_eq!(found.title, "2026年国庆节");
}

#[tokio::test]
async fn a_deleted_douyin_work_is_unreachable_and_a_bare_page_stays_a_page() {
    let deleted =
        r#"{"aweme_id":"1","filter_reason":"status_self_see","notice":"抱歉，作品不见了"}"#;
    let site = Site::start(vec![
        ("/ttwid/union/register/".to_string(), registered()),
        ("/share/video/1/".to_string(), share_page("", deleted)),
        ("/video/2".to_string(), web_page(false)),
    ])
    .await;
    let port = site.port;
    let links = previewer(&Store::new());
    assert_eq!(
        miss(&links, &format!("http://www.douyin.com:{port}/video/1")).await,
        Why::Unreachable
    );
    // 分享页拿不到（这台没有 /share/video/3/，404），兜底也 404：删了。
    assert_eq!(
        miss(&links, &format!("http://www.douyin.com:{port}/video/3")).await,
        Why::Unreachable
    );
    assert_eq!(
        site.hits("/video/3"),
        1,
        "兜底 404 就当删了，不再照普通网页抓一遍"
    );
    // 兜底的页面里也没有 `VideoObject`（改版、拦了）：退回普通网页。
    let plain = card(&links, &format!("http://www.douyin.com:{port}/video/2")).await;
    assert_eq!(plain.kind, Kind::Page);
    assert_eq!(plain.title, "抖音-记录美好生活");
}

#[tokio::test]
async fn douyin_short_links_are_followed_first() {
    let site = Site::start(vec![
        ("/ttwid/union/register/".to_string(), registered()),
        (format!("/share/video/{ID}/"), share_page(ITEM, "")),
        (
            "/?schema_type=20&object_id=1&is_qrcode=1".to_string(),
            web_page(false),
        ),
    ])
    .await;
    let port = site.port;
    let short = Site::start(vec![
        (
            "/L4FJNR3/".to_string(),
            Reply::redirect(&format!(
                "http://www.iesdouyin.com:{port}/share/video/{ID}/?region=US&from=web_code_link"
            )),
        ),
        (
            "/iRNBho6G/".to_string(),
            Reply::redirect(&format!(
                "http://www.iesdouyin.com:{port}/?schema_type=20&object_id=1&is_qrcode=1"
            )),
        ),
    ])
    .await;
    let links = previewer(&Store::new());
    let video = card(
        &links,
        &format!("http://v.douyin.com:{}/L4FJNR3/", short.port),
    )
    .await;
    assert_eq!(video.kind, Kind::Video);
    assert_eq!(
        video.url,
        format!("http://www.douyin.com:{port}/video/{ID}")
    );
    // 跳到首页的不是作品：照普通网页。
    let home = card(
        &links,
        &format!("http://v.douyin.com:{}/iRNBho6G/", short.port),
    )
    .await;
    assert_eq!(home.kind, Kind::Page);
}

#[tokio::test]
async fn a_tiktok_video_is_read_from_its_page_data() {
    let detail = format!(
        "{{\"itemInfo\":{{\"itemStruct\":{}}},\"shareMeta\":{{}},\"statusCode\":0,\"statusMsg\":\"\"}}",
        ITEM_STRUCT.trim()
    );
    let site = Site::start(vec![
        (
            "/@scout/video/7440114099565972791".to_string(),
            tiktok_page(&detail),
        ),
        (
            "/@/video/7440114099565972791".to_string(),
            tiktok_page(&detail),
        ),
        (
            "/@scout/video/1".to_string(),
            tiktok_page("{\"statusCode\":10204,\"statusMsg\":\"item doesn't exist\"}"),
        ),
        (
            "/@scout/video/2".to_string(),
            Reply::html("<html><head><title>Access Denied</title></head></html>"),
        ),
    ])
    .await;
    let port = site.port;
    let short = Site::start(vec![(
        "/ZMhvqjqjq/".to_string(),
        Reply::redirect(&format!(
            "http://www.tiktok.com:{port}/@/video/7440114099565972791?_r=1&_d=x"
        )),
    )])
    .await;
    let links = previewer(&Store::new());
    let page = format!("http://www.tiktok.com:{port}/@scout/video/7440114099565972791");
    let found = card(&links, &page).await;
    assert_eq!(found.kind, Kind::Video);
    assert!(
        found.title.starts_with("Scramble up ur name"),
        "{}",
        found.title
    );
    assert_eq!(found.author.as_deref(), Some("Scout, Suki & Stella"));
    assert_eq!(found.duration, Some(10));
    assert_eq!(found.site, "TikTok");
    assert_eq!(found.url, page);
    let seen = site
        .seen()
        .into_iter()
        .find(|s| s.target == "/@scout/video/7440114099565972791")
        .expect("读了视频页");
    assert!(
        seen.accept_language.starts_with("en-US"),
        "{}",
        seen.accept_language
    );
    // 短链跟完再认，卡片的地址去掉查询。
    let landed = card(
        &links,
        &format!("http://vm.tiktok.com:{}/ZMhvqjqjq/", short.port),
    )
    .await;
    assert_eq!(
        landed.url,
        format!("http://www.tiktok.com:{port}/@/video/7440114099565972791")
    );
    // 不存在的、没有数据（被拦了）的：不可达。
    for missing in ["/@scout/video/1", "/@scout/video/2"] {
        assert_eq!(
            miss(&links, &format!("http://www.tiktok.com:{port}{missing}")).await,
            Why::Unreachable,
            "{missing}"
        );
    }
}
