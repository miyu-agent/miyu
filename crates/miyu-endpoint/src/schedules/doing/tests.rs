//! 她写的那一句怎么清理（施工 LF-7）：换行换成空格、两头空白去掉、最多 200 个字（照字符，不照字节）。

use super::{MOST, cleaned};

#[test]
fn line_breaks_become_spaces_and_the_ends_are_trimmed() {
    assert_eq!(
        cleaned("  在打零式\n两小时后\r\n回来\r歇会儿\u{2028}再\u{2029}说 \n"),
        "在打零式 两小时后 回来 歇会儿 再 说"
    );
    assert_eq!(cleaned(" \n\t "), "");
}

#[test]
fn it_keeps_at_most_two_hundred_characters() {
    let long = "字".repeat(MOST + 5);
    let kept = cleaned(&long);
    assert_eq!(kept.chars().count(), MOST);
    assert_eq!(kept, "字".repeat(MOST));
    let exact = "a".repeat(MOST);
    assert_eq!(cleaned(&exact), exact, "正好 200 的不截");
}
