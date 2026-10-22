//! B 站视频先调接口（施工 W-7 四补，`net.md`「怎么走」第 12 条第 1 款第 2 条）：假服务器当 `api.bilibili.com`、
//! `www.bilibili.com`（协议、端口照视频页，所以是同一台）。

use crate::sites::{bilibili_page, find, json_reply, previewer};
use crate::support::{Reply, Site, Store, card, miss};
use miyu_net::{Kind, Why};
use serde_json::json;

/// 先调接口（施工 W-7 四补）：`User-Agent` 用不像浏览器的那个，`bvid`、`aid` 照路径，封面的 `http:` 换成 `https:`；取到了
/// 就不读页面。风控拦下的（HTTP 412、`code` 是 `-412`、`-352`）当过会儿再试、不读页面；别的 `code` 退回读页面。
#[tokio::test]
async fn a_bilibili_video_is_read_from_its_api_first() {
    let data = json!({"code": 0, "message": "OK", "data": {"title": "接口里的标题", "desc": "接口的简介",
        "pic": "http://i2.hdslb.com/bfs/archive/cover.jpg", "duration": 137,
        "owner": {"mid": 1, "name": "接口里的 UP 主"}}});
    let bili = Site::start(vec![
        (
            "/x/web-interface/view?bvid=BV1tWnv6KEfn".to_string(),
            json_reply(&data),
        ),
        (
            "/x/web-interface/view?aid=170001".to_string(),
            json_reply(
                &json!({"code": -404, "message": "啥都木有", "data": {"title": "不该用的"}}),
            ),
        ),
        (
            "/x/web-interface/view?aid=170002".to_string(),
            Reply::html("<html>412</html>").with_status(412),
        ),
        (
            "/x/web-interface/view?aid=170003".to_string(),
            json_reply(&json!({"code": -352, "message": "风控校验失败"})),
        ),
        (
            "/video/BV1tWnv6KEfn".to_string(),
            Reply::html("<html><head><title>拦截页</title></head></html>"),
        ),
        (
            "/video/av170001".to_string(),
            Reply::html(&bilibili_page("")),
        ),
    ])
    .await;
    let port = bili.port;
    let store = Store::new();
    let links = previewer(&store);

    let found = card(
        &links,
        &format!("http://www.bilibili.com:{port}/video/BV1tWnv6KEfn"),
    )
    .await;
    assert_eq!(found.title, "接口里的标题");
    assert_eq!(found.description, "接口的简介");
    assert_eq!(found.author.as_deref(), Some("接口里的 UP 主"));
    assert_eq!(found.duration, Some(137));
    assert_eq!(found.kind, Kind::Video);
    assert_eq!(found.site, "哔哩哔哩");
    let api = find(&bili, "/x/web-interface/view?bvid=");
    assert_eq!(api.len(), 1);
    assert_eq!(api[0].user_agent, "Miyu", "不像浏览器的才给");
    assert!(
        api[0].host.starts_with("api.bilibili.com"),
        "{}",
        api[0].host
    );
    assert!(
        find(&bili, "/video/BV1tWnv6KEfn").is_empty(),
        "取到了不读页面"
    );
    // 封面换成了 https：本机没有 443 上的图床，抓不到图，卡片照样成立
    assert!(found.image.is_none());

    let av = card(
        &links,
        &format!("http://www.bilibili.com:{port}/video/av170001"),
    )
    .await;
    assert_eq!(
        av.title, "页面上的标题_游戏热门视频",
        "别的 code 退回读页面"
    );
    assert_eq!(find(&bili, "/x/web-interface/view?aid=170001").len(), 1);
    for aid in ["170002", "170003"] {
        let url = format!("http://www.bilibili.com:{port}/video/av{aid}");
        assert_eq!(
            miss(&links, &url).await,
            Why::Unreachable,
            "风控拦下的过会儿再试：{aid}"
        );
        assert!(
            find(&bili, &format!("/video/av{aid}")).is_empty(),
            "不读页面"
        );
    }
}
