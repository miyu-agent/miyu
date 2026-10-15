//! 斜杠命令模板：读开头、照参数展开（施工 X-14 上）。

use super::*;

#[test]
fn names_follow_the_subcommand_rule() {
    for good in ["review", "fix-bug", "a1", "x"] {
        assert!(valid_name(good), "{good}");
    }
    let long = "a".repeat(33);
    for bad in [
        "",
        "Review",
        "1fix",
        "-x",
        "fix_bug",
        "fix.bug",
        "中文",
        long.as_str(),
    ] {
        assert!(!valid_name(bad), "{bad}");
    }
    assert_eq!(feature_id("review"), "command-review");
}

#[test]
fn the_front_matter_gives_the_human_words_and_the_rest_is_the_body() {
    let text = "---\ndescription: Review a file\nargument-hint: <file>\nallowed-tools: Bash(git:*)\nmodel: x\nmetadata:\n  summary_zh: 审一个文件\n---\nReview $ARGUMENTS carefully.\n";
    let read = read(text).expect("读得成");
    assert_eq!(
        read.summary,
        BTreeMap::from([
            ("en".to_string(), "Review a file".to_string()),
            ("zh".to_string(), "审一个文件".to_string()),
        ])
    );
    assert_eq!(read.argument.as_deref(), Some("<file>"));
    assert_eq!(read.body, "Review $ARGUMENTS carefully.\n");
}

#[test]
fn without_front_matter_the_whole_file_is_the_body() {
    for text in [
        "Just do it.\n",
        "\u{feff}Just do it.\n",
        "--- not a fence\nbody",
    ] {
        let read = read(text).expect("读得成");
        assert!(read.summary.is_empty());
        assert_eq!(read.argument, None);
        assert_eq!(read.body, text.trim_start_matches('\u{feff}'));
    }
    // 只有开头一行 `---`、没有收尾的：当没有开头，整份是正文。
    assert_eq!(read("---\nbody\n").expect("读得成").body, "---\nbody\n");
    // 空的开头。
    let empty = read("---\n---\nbody\n").expect("读得成");
    assert_eq!((empty.summary.len(), empty.body.as_str()), (0, "body\n"));
}

#[test]
fn a_broken_front_matter_says_why() {
    assert!(matches!(
        read("---\ndescription: [unclosed\n---\nbody\n"),
        Err(TemplateProblem::Yaml(_))
    ));
    assert_eq!(
        read("---\n- a\n- b\n---\nbody\n"),
        Err(TemplateProblem::NotAMap)
    );
}

#[test]
fn arguments_fill_the_placeholders() {
    assert_eq!(
        expand("Review $ARGUMENTS now.\n", "  src/a.rs  b.rs "),
        "Review src/a.rs  b.rs now."
    );
    assert_eq!(
        expand(
            "From $ARGUMENTS[0] to $ARGUMENTS[1], not $ARGUMENTS[5].",
            "a b"
        ),
        "From a to b, not ."
    );
    // 不是数字的方括号照原样留着，前面的换成整段。
    assert_eq!(expand("$ARGUMENTS[x]", "a"), "a[x]");
    // 参数里带记号的不再展开。
    assert_eq!(expand("[$ARGUMENTS]", "$ARGUMENTS"), "[$ARGUMENTS]");
    // 用了记号、没跟字的换成空的。
    assert_eq!(expand("Go $ARGUMENTS.", ""), "Go .");
}

#[test]
fn arguments_without_a_placeholder_go_at_the_end() {
    assert_eq!(
        expand("Summarize the diff.\n\n", "focus on tests"),
        "Summarize the diff.\n\nfocus on tests"
    );
    assert_eq!(expand("Summarize the diff.\n", ""), "Summarize the diff.");
    assert_eq!(expand("   \n", ""), "");
}
