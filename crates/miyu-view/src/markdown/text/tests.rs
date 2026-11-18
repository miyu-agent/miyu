//! 正文原样写进折叠块（审计 #021 第 1 条，施工 AU-P1）：哪一行在代码块里、哪一行不在，照围栏真开没开、真合没合判。

use super::prose;

/// 开头三个反引号、后面的字里又有反引号的那一行不是围栏（CommonMark 的规矩），是普通的一行：里面的 `</details>` 照样写成
/// `&lt;/details>`，不然它把折叠块提前合上。真的围栏行、代码块里面的照原样。
#[test]
fn a_backtick_line_that_is_not_a_fence_is_still_escaped() {
    let text = "```inline`</details>\nafter\n";
    assert_eq!(
        prose(text, true),
        "```inline`&lt;/details>\nafter\n",
        "不是围栏的那一行照中和"
    );
    let fenced = "```\n</details>\n```\n</details>\n";
    assert_eq!(
        prose(fenced, true),
        "```\n</details>\n```\n&lt;/details>\n",
        "代码块里面的照原样，合上以后的照中和"
    );
    assert_eq!(prose(text, false), text, "不在折叠块里的不动");
}
