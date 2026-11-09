//! 新密钥的名字（施工 BP-3，同网页的 `secretName`）：照键起、小写、别的字并成一个 `-`，带上这一刻，最多 64 个字。

use super::{base36, secret_name};

#[test]
fn a_secret_is_named_after_its_key() {
    let name = secret_name("Relay.Token__Main");
    assert!(name.starts_with("relay-token-main-"), "{name}");
    assert!(
        name.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
        "{name}"
    );
    assert!(secret_name(&"k".repeat(100)).chars().count() <= 64);
    assert_eq!(
        (base36(0), base36(35), base36(36)),
        ("0".to_string(), "z".to_string(), "10".to_string())
    );
}
