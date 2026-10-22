//! 为什么护不住：没有 bubblewrap 的先说；有的看沙盒强度。

use super::*;

#[test]
fn the_reason_is_no_bwrap_first_then_weak() {
    assert_eq!(reason(false, "auto"), Some("no_bwrap"));
    assert_eq!(reason(false, WEAK), Some("no_bwrap"), "没有的不说弱沙盒");
    assert_eq!(reason(true, WEAK), Some("weak"));
    assert_eq!(reason(true, "auto"), None, "有挂载、不是弱沙盒：护得住");
}
