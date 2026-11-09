//! 程序自己起的服务的网址（施工 BP-2）：片段是 JSON 的 base64url，不带 `=`，同网页 `external.js`。

use super::{base64url, site_url};

#[test]
fn the_fragment_is_base64url_of_the_handed_json() {
    assert_eq!(base64url(b"hi"), "aGk");
    assert_eq!(base64url(b"\xff\xfe\xfd"), "__79");
    assert_eq!(base64url(b"\xfb"), "-w");
    assert_eq!(base64url(b""), "");
    let url = site_url(4321, "t", "zh");
    assert_eq!(
        url,
        format!(
            "http://127.0.0.1:4321/#miyu={}",
            base64url(br#"{"lang":"zh","ticket":"t","v":1}"#)
        )
    );
}
