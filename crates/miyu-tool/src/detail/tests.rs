//! 三种通用形状：照形状读、上限、多一格不要（施工 W-11 补）。

use serde_json::json;

use super::{Detail, MOST_LINKS, read};

const HASH: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

#[test]
fn the_three_shapes_are_read_and_written_back_as_they_were() {
    for value in [
        json!({"kind": "links", "links": [{"title": "Miyu", "url": "https://example.com", "snippet": "A cat.", "site": "Example", "date": "2026-10-18"}]}),
        json!({"kind": "table", "columns": ["城市", "气温"], "rows": [["上海", 21.5], ["北京", null]]}),
        json!({"kind": "images", "images": [{"blob": HASH, "media_type": "image/png", "caption": "晚霞"}]}),
    ] {
        let detail = read(&value).unwrap_or_else(|why| panic!("{why}: {value}"));
        assert_eq!(detail.to_value(), value);
    }
}

#[test]
fn only_images_reference_blobs() {
    let images =
        read(&json!({"kind": "images", "images": [{"blob": HASH, "media_type": "image/jpeg"}]}))
            .unwrap();
    assert_eq!(images.blobs().len(), 1);
    let links = read(&json!({"kind": "links", "links": []})).unwrap();
    assert!(links.blobs().is_empty());
}

#[test]
fn a_detail_out_of_shape_is_refused_with_a_reason() {
    let many: Vec<_> = (0..=MOST_LINKS)
        .map(|n| json!({"title": n.to_string(), "url": "https://example.com"}))
        .collect();
    for (value, why) in [
        (json!({"kind": "chart"}), "unknown variant"),
        (json!({"links": []}), "missing field `kind`"),
        (
            json!({"kind": "links", "links": [], "extra": 1}),
            "unknown field",
        ),
        (
            json!({"kind": "links", "links": [{"title": "x", "url": "file:///etc/passwd"}]}),
            "not an http",
        ),
        (
            json!({"kind": "links", "links": [{"title": "x", "url": "https://"}]}),
            "not an http",
        ),
        (
            json!({"kind": "links", "links": [{"title": "x", "url": "https://a", "color": "red"}]}),
            "unknown field",
        ),
        (
            json!({"kind": "links", "links": many}),
            "more than 50 links",
        ),
        (
            json!({"kind": "table", "columns": [], "rows": []}),
            "columns must be",
        ),
        (
            json!({"kind": "table", "columns": ["a", "b"], "rows": [["1"]]}),
            "one cell per column",
        ),
        (
            json!({"kind": "table", "columns": ["a"], "rows": [[["nested"]]]}),
            "not a string",
        ),
        (
            json!({"kind": "images", "images": [{"blob": "not-a-hash", "media_type": "image/png"}]}),
            "content hash",
        ),
        (
            json!({"kind": "images", "images": [{"blob": HASH, "media_type": "application/pdf"}]}),
            "not an image",
        ),
        (
            json!({"kind": "links", "links": [{"title": "x".repeat(70_000), "url": "https://a"}]}),
            "more than 65536",
        ),
    ] {
        let error = read(&value).expect_err(&value.to_string()[..60.min(value.to_string().len())]);
        assert!(error.contains(why), "{why}：{error}");
    }
}

#[test]
fn a_built_in_tool_writes_the_same_shape() {
    let detail = Detail::Links { links: Vec::new() };
    assert_eq!(detail.to_value(), json!({"kind": "links", "links": []}));
}
