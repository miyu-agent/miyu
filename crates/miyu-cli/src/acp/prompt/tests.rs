use serde_json::json;

use super::{local_path, read};

#[test]
fn text_links_images_and_resources_are_read_in_order() {
    let prompt = json!([
        {"type": "text", "text": "看看"},
        {"type": "resource_link", "uri": "file:///home/me/my%20notes.md", "name": "my notes.md"},
        {"type": "text", "text": "和这张图"},
        {"type": "image", "mimeType": "image/jpeg", "data": "aGk="},
        {"type": "resource", "resource": {"uri": "file:///w/src/main.rs", "mimeType": "text/x-rust", "text": "fn main() {}\n"}},
        {"type": "resource", "resource": {"uri": "zed://selection", "blob": "AAE="}},
        {"type": "audio", "mimeType": "audio/wav", "data": "aGk="},
    ]);
    let said = read(&prompt).expect("读得懂");
    let path = local_path("file:///home/me/my%20notes.md").expect("本机文件");
    assert_eq!(said.text, format!("看看 {path} 和这张图"));
    assert_eq!(said.files.len(), 3);
    assert_eq!(said.files[0].name, "image-1.jpg");
    assert_eq!(said.files[0].bytes, b"hi");
    assert_eq!(said.files[1].name, "main.rs");
    assert_eq!(said.files[1].media_type.as_deref(), Some("text/x-rust"));
    assert_eq!(said.files[1].bytes, b"fn main() {}\n");
    assert_eq!(said.files[2].name, "selection");
    assert_eq!(said.files[2].bytes, [0, 1]);
}

#[test]
fn bad_base64_refuses_the_whole_prompt() {
    let prompt = json!([{"type": "image", "mimeType": "image/png", "data": "不是 base64"}]);
    assert!(read(&prompt).is_err());
}

#[test]
fn file_uris_become_paths() {
    assert_eq!(
        local_path("file:///tmp/a%23b.txt").as_deref(),
        Some("/tmp/a#b.txt")
    );
    assert_eq!(
        local_path("file://localhost/tmp/x").as_deref(),
        Some("/tmp/x")
    );
    assert_eq!(local_path("https://example.com/x"), None);
    let windows = local_path("file:///C:/Users/me/a.txt").expect("本机文件");
    match cfg!(windows) {
        true => assert_eq!(windows, "C:/Users/me/a.txt"),
        false => assert_eq!(windows, "/C:/Users/me/a.txt"),
    }
}
