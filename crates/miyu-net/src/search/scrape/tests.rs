//! 两家结果页的读法（施工 N-3 下）：样本照 2026-10-19 真页面的结构删短，摘录的字是编的（LIC-3，发布前审计：不随源码发别人的内容），链接是真的包法，人机验证页当限额。

use super::{bing, duckduckgo};
use crate::search::Failure;

const DUCKDUCKGO: &str = r#"<html><body><div class="serp__results"><div id="links" class="results">
<div class="result results_links results_links_deep web-result "><div class="links_main links_deep result__body">
<h2 class="result__title"><a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fdocs.rs%2Ftokio%2Flatest%2Ftokio%2Fmacro.select.html&amp;rut=abc"><b>select</b> in tokio - Rust</a></h2>
<div class="result__extras"><div class="result__extras__url"><a class="result__url" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fdocs.rs">docs.rs/tokio</a></div></div>
<a class="result__snippet" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fdocs.rs">Waits on multiple concurrent   branches.</a>
<div class="result__snippet">a second snippet block</div>
</div></div>
<div class="result results_links web-result "><div class="links_main result__body">
<h2 class="result__title"><a rel="nofollow" class="result__a" href="https://tokio.rs/tokio/tutorial/select">Select | Tokio</a></h2>
</div></div>
</div></div></body></html>"#;

const BING: &str = r#"<html><body><ol id="b_results">
<li class="b_algo" data-id iid=SERP.5351><div class="b_tpcn"><a class="tilk" aria-label="wikipedia.org" href="https://www.bing.com/ck/a?!&amp;&amp;p=bc9b&amp;ptn=3&amp;ver=2&amp;hsh=4&amp;u=a1aHR0cHM6Ly9qYS53aWtpcGVkaWEub3JnL3dpa2kvVE9LSU8&amp;ntb=1"><div class="tptt">wikipedia.org</div></a></div><h2><a target="_blank" href="https://www.bing.com/ck/a?!&amp;&amp;p=bc9b&amp;ptn=3&amp;ver=2&amp;hsh=4&amp;u=a1aHR0cHM6Ly9qYS53aWtpcGVkaWEub3JnL3dpa2kvVE9LSU8&amp;ntb=1" h="ID=SERP,5129.2"><strong>TOKIO</strong> - Wikipedia</a></h2><div class="b_caption"><p class="b_lineclamp2">示例摘要：这一段是编出来的，只留结果页的结构 …<a href="https://ja.wikipedia.org/wiki/TOKIO">阅读更多</a></p></div></li>
<li class="b_algo"><h2><a href="https://docs.rs/tokio">tokio - Rust</a></h2></li>
<li class="b_ad"><h2><a href="https://ads.example/">An ad</a></h2></li>
</ol></body></html>"#;

#[test]
fn duckduckgo_results_are_unwrapped_and_snippets_follow_their_title() {
    let hits = duckduckgo(DUCKDUCKGO).unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].title, "select in tokio - Rust");
    assert_eq!(
        hits[0].url, "https://docs.rs/tokio/latest/tokio/macro.select.html",
        "uddg 解码"
    );
    assert_eq!(
        hits[0].snippet.as_deref(),
        Some("Waits on multiple concurrent branches."),
        "空白收拢"
    );
    assert_eq!(
        hits[1].url, "https://tokio.rs/tokio/tutorial/select",
        "没包的照原样"
    );
    assert_eq!(hits[1].snippet, None, "没有摘录的不借上一条的");
}

#[test]
fn bing_results_are_unwrapped_from_base64_and_ads_are_not_results() {
    let hits = bing(BING).unwrap();
    assert_eq!(hits.len(), 2, "广告不是 b_algo");
    assert_eq!(hits[0].title, "TOKIO - Wikipedia");
    assert_eq!(
        hits[0].url, "https://ja.wikipedia.org/wiki/TOKIO",
        "u=a1 后面的 base64 解开"
    );
    let snippet = hits[0].snippet.as_deref().unwrap();
    assert!(
        snippet.starts_with("示例摘要") && !snippet.contains("阅读更多"),
        "摘录里不要展开的链接：{snippet}"
    );
    assert_eq!(hits[1].url, "https://docs.rs/tokio");
}

#[test]
fn a_challenge_page_is_a_limit_and_an_empty_page_is_nothing() {
    let challenge = r#"<html><body><form id="challenge-form"><script src="/anomaly.js?sv=html&cc=botnet"></script></form></body></html>"#;
    assert_eq!(duckduckgo(challenge), Err(Failure::Limited));
    assert_eq!(
        bing("<html><body><div class=\"captcha\">verify</div></body></html>"),
        Err(Failure::Limited)
    );
    assert_eq!(
        duckduckgo("<html><body>No results.</body></html>").unwrap(),
        [],
        "没有结果不算出错"
    );
}
