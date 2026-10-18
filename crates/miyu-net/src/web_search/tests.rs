//! 给头看的站点（施工 N-2 补）：网址的主机名，去掉开头的 `www.`，小写；读不成的没有。

use super::site_of;

#[test]
fn the_site_is_the_host_without_www() {
    assert_eq!(
        site_of("https://www.GitHub.com/rust-lang/rust").as_deref(),
        Some("github.com")
    );
    assert_eq!(site_of("https://docs.rs/tokio").as_deref(), Some("docs.rs"));
    assert_eq!(
        site_of("https://wwwx.example.org/").as_deref(),
        Some("wwwx.example.org"),
        "只去掉 www. 这一段"
    );
    assert_eq!(site_of("not a url"), None);
}
