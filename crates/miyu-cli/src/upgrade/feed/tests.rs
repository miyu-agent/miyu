//! 订阅里的标签：照 GitHub 的样子认，正文里的链接不算，不是订阅的说不是。

use super::*;

/// 2026-11-28 从 `https://github.com/miyu-agent/miyu/releases.atom` 取回来的头、尾和前两个 entry。正文截短了，提交的链接换成了指向
/// 别的标签的链接：转义过的不该被认出来。
const REAL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:media="http://search.yahoo.com/mrss/" xml:lang="en-US">
  <id>tag:github.com,2008:https://github.com/miyu-agent/miyu/releases</id>
  <link type="text/html" rel="alternate" href="https://github.com/miyu-agent/miyu/releases"/>
  <link type="application/atom+xml" rel="self" href="https://github.com/miyu-agent/miyu/releases.atom"/>
  <title>Release notes from miyu</title>
  <updated>2026-11-27T14:50:13Z</updated>
  <entry>
    <id>tag:github.com,2008:Repository/1219967630/v2026.11.0-rc.5</id>
    <updated>2026-11-27T15:27:59Z</updated>
    <link rel="alternate" type="text/html" href="https://github.com/miyu-agent/miyu/releases/tag/v2026.11.0-rc.5"/>
    <title>Miyu v2026.11.0-rc.5</title>
    <content type="html">&lt;p&gt;草稿：v2026.11.0-rc.5（打包的提交 &lt;a class=&quot;commit-link&quot; href=&quot;https://github.com/miyu-agent/miyu/releases/tag/v0.7.4&quot;&gt;&lt;tt&gt;015f83b&lt;/tt&gt;&lt;/a&gt;）。&lt;/p&gt;</content>
    <author>
      <name>shorinkiwata</name>
    </author>
  </entry>
  <entry>
    <id>tag:github.com,2008:Repository/1219967630/v2026.11.0-rc.4</id>
    <updated>2026-11-26T09:00:00Z</updated>
    <link rel="alternate" type="text/html" href="https://github.com/miyu-agent/miyu/releases/tag/v2026.11.0-rc.4"/>
    <title>Miyu v2026.11.0-rc.4</title>
    <content type="html">&lt;p&gt;rc.4&lt;/p&gt;</content>
  </entry>
</feed>
"#;

#[test]
fn the_tags_come_from_each_entrys_release_link() {
    assert_eq!(
        tags(REAL.as_bytes()),
        Some(vec![
            "v2026.11.0-rc.5".to_string(),
            "v2026.11.0-rc.4".to_string()
        ]),
        "正文里转义过的 v0.7.4 链接、订阅头上的链接都不算"
    );
}

#[test]
fn an_entry_without_a_release_link_is_left_out() {
    let text = concat!(
        "<feed><entry><link href=\"https://github.com/x/y/releases\"/></entry>",
        "<entry ><link rel=\"alternate\" href=\"https://github.com/x/y/releases/tag/v1.0.0\"/></entry>",
        "<entries/></feed>"
    );
    assert_eq!(tags(text.as_bytes()), Some(vec!["v1.0.0".to_string()]));
}

#[test]
fn an_empty_feed_has_no_tags_and_anything_else_is_not_a_feed() {
    assert_eq!(
        tags(b"<?xml version=\"1.0\"?><feed xmlns=\"http://www.w3.org/2005/Atom\"></feed>"),
        Some(Vec::new())
    );
    for bad in [
        &b""[..],
        b"<html><body>rate limited</body></html>",
        b"<feed><entry>",
        b"\xff\xfe<feed></feed>",
    ] {
        assert_eq!(tags(bad), None, "{:?}", String::from_utf8_lossy(bad));
    }
}
