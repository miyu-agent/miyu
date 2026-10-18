//! 分页（施工 N-1）：每页不超过上限、拼回去一个字不差、尽量断在换行上、断不了的照字数断。

use super::split;

#[test]
fn pages_put_back_together_are_the_whole_text_and_none_is_over_the_limit() {
    let text: String = (0..500).map(|n| format!("第 {n} 行，line {n}\n")).collect();
    let pages = split(&text, 1000);
    assert!(pages.len() > 1);
    assert_eq!(pages.concat(), text);
    assert!(pages.iter().all(|page| page.chars().count() <= 1000));
    assert!(
        pages[..pages.len() - 1]
            .iter()
            .all(|page| page.ends_with('\n')),
        "能断在换行上的断在换行上"
    );
}

#[test]
fn a_text_without_newlines_is_cut_by_count_and_an_empty_one_is_one_page() {
    let text = "字".repeat(25);
    let pages = split(&text, 10);
    assert_eq!(
        pages
            .iter()
            .map(|page| page.chars().count())
            .collect::<Vec<_>>(),
        [10, 10, 5]
    );
    assert_eq!(split("", 10), [""]);
}

#[test]
fn a_newline_too_early_in_the_page_is_not_used() {
    // 换行在这一页的前五分之四里：照它断就太短了，照字数断。
    let text = format!("ab\n{}", "c".repeat(20));
    let pages = split(&text, 10);
    assert_eq!(pages[0], format!("ab\n{}", "c".repeat(7)));
}
