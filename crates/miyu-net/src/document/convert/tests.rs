//! 认种类、解字符集、换 Markdown（施工 N-1）。

use super::{Kind, Unconverted, decode, kind, markdown, tidy, title};

#[test]
fn the_kind_follows_the_media_type_and_an_unnamed_one_is_guessed() {
    assert_eq!(kind("text/html", b""), Some(Kind::Html));
    assert_eq!(kind("application/xhtml+xml", b""), Some(Kind::Html));
    assert_eq!(kind("text/plain", b""), Some(Kind::Text));
    assert_eq!(kind("application/json", b""), Some(Kind::Text));
    assert_eq!(kind("application/ld+json", b""), Some(Kind::Text));
    assert_eq!(kind("image/png", b"\x89PNG"), None);
    assert_eq!(kind("application/pdf", b"%PDF"), None);
    assert_eq!(
        kind("", b"\xef\xbb\xbf  <!DOCTYPE html><html>"),
        Some(Kind::Html)
    );
    assert_eq!(
        kind("application/octet-stream", b"just words"),
        Some(Kind::Text)
    );
    assert_eq!(kind("", b"\x00\x01\x02"), None, "有 NUL 的不当文字");
}

#[test]
fn the_charset_comes_from_the_header_then_the_meta_tag_then_utf8() {
    // 「中文」的 GBK 编码
    let gbk = b"\xd6\xd0\xce\xc4";
    assert_eq!(decode(gbk, Some("gbk"), Kind::Text), "中文");
    let page = [
        &b"<html><head><meta charset=\"GBK\"></head><body>"[..],
        gbk,
        b"</body></html>",
    ]
    .concat();
    assert!(decode(&page, None, Kind::Html).contains("中文"));
    let quoted = [
        &b"<meta http-equiv=\"Content-Type\" content=\"text/html; charset=gb2312\">"[..],
        gbk,
    ]
    .concat();
    assert!(decode(&quoted, None, Kind::Html).ends_with("中文"));
    assert_eq!(decode("中文".as_bytes(), None, Kind::Html), "中文");
    assert_eq!(
        decode("中文".as_bytes(), Some("no-such-charset"), Kind::Text),
        "中文"
    );
}

#[test]
fn the_title_is_collapsed_and_unescaped() {
    assert_eq!(
        title("<html><head><TITLE>\n  Miyu &amp;\n friends </TITLE></head></html>").as_deref(),
        Some("Miyu & friends")
    );
    assert_eq!(title("<title>  </title>"), None);
    assert_eq!(title("<p>no title</p>"), None);
}

#[test]
fn html_becomes_markdown_without_scripts_navigation_or_forms() {
    let html = r#"<html><head><title>T</title><style>p{color:red}</style></head><body>
        <nav><a href="/">Home</a></nav>
        <h1>Heading</h1><p>Some <strong>bold</strong> and a <a href="https://example.com/x">link</a>.</p>
        <script>alert("no")</script><form><input name="q"><button>Go</button></form>
        <ul><li>one</li><li>two</li></ul>
        <footer>copyright</footer></body></html>"#;
    let text = markdown(html).expect("换得成");
    assert!(text.starts_with("# Heading"), "{text}");
    assert!(text.contains("**bold**"), "{text}");
    assert!(text.contains("[link](https://example.com/x)"), "{text}");
    assert!(text.contains("one") && text.contains("two"), "{text}");
    for gone in ["alert", "color:red", "Home", "copyright", "Go", "T\n"] {
        assert!(!text.contains(gone), "{gone} 该扔掉：{text}");
    }
}

#[test]
fn a_page_nested_too_deep_is_not_converted() {
    let deep = format!("{}x{}", "<div>".repeat(5000), "</div>".repeat(5000));
    assert_eq!(markdown(&deep), Err(Unconverted::TooComplex));
    let fine = format!("{}x{}", "<div>".repeat(50), "</div>".repeat(50));
    assert_eq!(markdown(&fine).as_deref(), Ok("x"));
}

#[test]
fn tidy_trims_line_ends_and_folds_blank_runs() {
    assert_eq!(tidy("\n\n a  \n\n\n\nb\t\n\n"), "a\n\nb");
}

#[test]
fn only_the_main_part_is_read_when_there_is_one() {
    let html = r##"<html><body><div class="langs"><a href="/de">Deutsch</a></div>
        <main><h1>Topic</h1><p>Body <a href="#top"><span></span></a>text.</p></main>
        <div class="side">Sidebar</div></body></html>"##;
    assert_eq!(markdown(html).as_deref(), Ok("# Topic\n\nBody text."));
    let one_article = "<div>Header</div><article><p>Story</p></article><div>More</div>";
    assert_eq!(markdown(one_article).as_deref(), Ok("Story"));
    let two_articles = "<article><p>One</p></article><article><p>Two</p></article>";
    assert_eq!(
        markdown(two_articles).as_deref(),
        Ok("One\n\nTwo"),
        "两篇的整份都要"
    );
}

#[test]
fn hidden_parts_landmarks_and_language_lists_are_pruned() {
    let html = r#"<main><h1>Topic</h1>
        <ul><li><a href="https://de.example.org/T" hreflang="de">Deutsch</a></li>
            <li><a href="https://fr.example.org/T" hreflang="fr">Français</a></li></ul>
        <div hidden>secret</div><span aria-hidden="true">icon</span>
        <div role="navigation">Prev Next</div><div role="search">Find</div>
        <ul><li>kept item</li></ul><p aria-hidden="false">kept text</p></main>"#;
    let text = markdown(html).expect("换得成");
    assert_eq!(text, "# Topic\n\n*   kept item\n\nkept text", "{text}");
}
