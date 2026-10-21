//! 读出来的资源（施工 X-41）：类型是图的二进制换成图，别的原样。

use super::*;

fn resource(mime: Option<&str>, text: Option<&str>, blob: Option<&str>) -> Content {
    Content::Resource {
        uri: "file:///x".to_string(),
        mime: mime.map(str::to_string),
        text: text.map(str::to_string),
        blob: blob.map(str::to_string),
    }
}

#[test]
fn an_image_blob_becomes_a_picture_and_the_rest_stays() {
    assert_eq!(
        picture(resource(Some("image/png"), None, Some("aGk="))),
        Content::Image {
            data: "aGk=".to_string(),
            mime: "image/png".to_string()
        }
    );
    let binary = resource(Some("application/octet-stream"), None, Some("AAEC"));
    assert_eq!(
        picture(binary.clone()),
        binary,
        "别的二进制照资源，换块时写一行"
    );
    let text = resource(Some("image/svg+xml"), Some("<svg/>"), None);
    assert_eq!(picture(text.clone()), text, "写成字的照字");
    let untyped = resource(None, None, Some("aGk="));
    assert_eq!(picture(untyped.clone()), untyped, "没写类型的不猜");
}
