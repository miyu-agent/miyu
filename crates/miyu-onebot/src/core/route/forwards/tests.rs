//! 转发里一条的正文怎么截（施工 O-34，核心审时建议）：先照字符数截，再夹到核心的 4096 字节，截在字的边界上；`forward_chars`
//! 调大了、全是 4 字节的字也不超。

use super::{TEXT_BYTES, clip};

#[test]
fn text_is_cut_by_characters_then_held_to_the_core_bytes() {
    assert_eq!(clip("长".repeat(600).as_str(), 500), "长".repeat(500));
    // 全是 4 字节的字、`forward_chars` 调到 2000：夹到 1024 个字、正好 4096 字节。
    let wide = "😀".repeat(2000);
    let cut = clip(&wide, 2000);
    assert_eq!(cut, "😀".repeat(1024));
    assert_eq!(cut.len(), TEXT_BYTES);
    // 边界不齐的：一个 1 字节的字在前，4 字节的放不下半个，停在 4093 字节。
    let odd = format!("a{}", "😀".repeat(2000));
    let cut = clip(&odd, 5000);
    assert_eq!(cut.len(), 1 + 4 * 1023);
    assert!(cut.ends_with('😀'));
    assert_eq!(clip("短", 0), "");
}
