use super::*;

/// 访问密码多短都行（施工 PW-1，项目主人 2026-10-30）：只挡空的、全是空白的、超过 1024 个字节的。
#[test]
fn any_password_that_is_not_blank_or_huge_is_good() {
    for good in ["a", "1", "密", "pw", &"x".repeat(LONGEST)] {
        assert!(good_password(good), "{good:?}");
    }
    for bad in ["", " ", "  \t\n", &"x".repeat(LONGEST + 1)] {
        assert!(!good_password(bad), "{bad:?}");
    }
}
