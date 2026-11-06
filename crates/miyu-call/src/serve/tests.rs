//! 令牌、端点名（施工 XG-1补2）：令牌照等时比较认，端点名不带令牌。

use super::*;

#[test]
fn only_the_same_token_matches() {
    let token = random_hex().expect("有随机数");
    assert!(same_token(&token, &token));
    assert!(same_token(&token.clone(), &token));
    // 差在头、尾、中间的，长了、短了的，空的：都不对。
    let mut first = token.clone().into_bytes();
    first[0] ^= 1;
    let mut last = token.clone().into_bytes();
    last[31] ^= 1;
    let mut middle = token.clone().into_bytes();
    middle[15] ^= 0x20;
    for wrong in [first, last, middle] {
        assert!(!same_token(
            &String::from_utf8(wrong).expect("还是字"),
            &token
        ));
    }
    assert!(!same_token(&token[..31], &token));
    assert!(!same_token(&format!("{token}0"), &token));
    assert!(!same_token("", &token));
    assert!(same_token("", ""));
}

#[test]
fn an_endpoint_tag_is_its_own_random_string() {
    let (a, b) = (
        endpoint_tag().expect("有随机数"),
        endpoint_tag().expect("有随机数"),
    );
    assert_eq!(a.len(), 16);
    assert!(a.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_ne!(a, b);
}
