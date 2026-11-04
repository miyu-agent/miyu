use super::*;

#[test]
fn each_line_reads_back_as_written() {
    for ready in [
        Ready::Ready,
        Ready::Running,
        Ready::failed("找不到资源目录"),
        Ready::Failed {
            code: Some("data_root_not_ours".to_string()),
            reason: "/home/me/x 里有别的东西".to_string(),
        },
    ] {
        let line = ready.line();
        assert!(
            line.ends_with('\n') && line.matches('\n').count() == 1,
            "{line:?}"
        );
        assert_eq!(Ready::parse(&line), ready);
    }
}

#[test]
fn a_reason_stays_on_one_line() {
    let line = Ready::failed("第一行\r\n第二行").line();
    assert_eq!(line, "error 第一行  第二行\n");
}

#[test]
fn a_line_not_understood_is_a_failure_that_says_it() {
    assert_eq!(
        Ready::parse("panicked at main.rs\n"),
        Ready::failed("panicked at main.rs")
    );
}

/// 施工 LC-1：冒号前合写法的才是代码；不合的（大写、空格、中文、空的）整句当原因，以前的核心写的照旧读。
#[test]
fn only_a_well_formed_word_before_the_colon_is_a_code() {
    assert_eq!(
        Ready::parse("error data_root_old_layout: 旧版\n"),
        Ready::Failed {
            code: Some("data_root_old_layout".to_string()),
            reason: "旧版".to_string()
        }
    );
    for reason in [
        "Permission denied: /x",
        "拉不起: x",
        "two words: x",
        ": x",
        "9lives: x",
        "no colon here",
    ] {
        assert_eq!(
            Ready::parse(&format!("error {reason}\n")),
            Ready::failed(reason),
            "{reason}"
        );
    }
}
