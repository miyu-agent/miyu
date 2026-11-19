//! 供应商自己的联网工具在路由这一头记着的两样（设计 72，施工 DSR-3）。

use miyu_models::provider_search::ProviderSearch;

use super::{Held, explicit};

fn names() -> Vec<String> {
    vec![
        "deepseek_web_search".to_string(),
        "deepseek_read_url".to_string(),
    ]
}

/// 快照钉着要声明的，只在这一次的端点给得了的时候带上：出错换到了给不了的端点的那一次不声明。
#[test]
fn only_an_endpoint_that_offers_it_gets_the_declared_tools() {
    let held = Held::default();
    assert!(held.hosted(true).is_empty(), "没钉的不声明");
    held.declare(names());
    assert_eq!(held.hosted(true), names());
    assert!(held.hosted(false).is_empty(), "这个端点给不了的不声明");
    held.declare(Vec::new());
    assert!(held.hosted(true).is_empty(), "换了快照、不钉了的不再声明");
}

/// 回合开始记下的那一份，写成策略那边认的样子；给不了的没有。
#[test]
fn the_offer_of_the_turn_is_handed_to_the_policy() {
    let held = Held::default();
    assert_eq!(held.offered(), None, "还没开过回合");
    held.offer(Some(ProviderSearch {
        setting: "deepseek-responses".to_string(),
        driver: "openai-responses".to_string(),
        effort: "high".to_string(),
        replaces: vec![("web_search".to_string(), "deepseek_web_search".to_string())],
    }));
    let offered = held.offered().expect("给得了");
    assert_eq!(offered.setting, "deepseek-responses");
    assert_eq!(
        offered.replaces,
        [("web_search".to_string(), "deepseek_web_search".to_string())]
    );
    held.offer(None);
    assert_eq!(held.offered(), None);
}

/// 声明了供应商侧工具、这一次又没有定思考强度的，照档案写的那一档明着写（DeepSeek 不明着写不会真去搜索）；定了强度的、
/// 没声明的不改。
#[test]
fn a_declared_request_without_a_level_gets_the_profile_level() {
    assert_eq!(
        explicit(&names(), None, Some("high")),
        Some("high".to_string())
    );
    assert_eq!(
        explicit(&names(), Some("low"), Some("high")),
        None,
        "定了的照定的"
    );
    assert_eq!(explicit(&[], None, Some("high")), None, "没声明的不改");
    assert_eq!(explicit(&names(), None, None), None, "这个端点给不了的不改");
}
