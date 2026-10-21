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

#[test]
fn words_are_cut_at_whitespace_and_a_quoted_pair_is_one_word() {
    assert_eq!(
        words("  a  b\tc\n"),
        ["a", "b", "c"],
        "照空白切，前后空白、换行不算"
    );
    assert_eq!(
        words("\"San Francisco\" CA"),
        ["San Francisco", "CA"],
        "第 1 条：成对的英文双引号中间是一个词，引号去掉"
    );
    assert_eq!(
        words("\"\" CA"),
        ["", "CA"],
        "第 1 条：空的引号是一个空的词"
    );
    assert_eq!(
        words("\"a\" \"b c\""),
        ["a", "b c"],
        "第 1 条：到下一个引号为止，不是最后一个"
    );
    assert_eq!(
        next_word("  a   b c "),
        Some(("a", "b c ")),
        "交回的剩下的去掉开头的空白"
    );
    assert_eq!(next_word("\"a b\"   c"), Some(("a b", "c")));
    assert_eq!(
        words("a\"b c\" d\""),
        ["a\"b", "c\"", "d\""],
        "第 2 条：词中间、末尾的引号照原样"
    );
    assert_eq!(
        words("\"open ended"),
        ["\"open", "ended"],
        "第 2 条：开头的引号没有配对的照原样"
    );
    assert_eq!(
        words("\\\"a b\\\""),
        ["\\\"a", "b\\\""],
        "第 3 条：不做转义"
    );
    assert_eq!(
        words("“上海 浦东” 「北京」"),
        ["“上海", "浦东”", "「北京」"],
        "第 3 条：中文引号照普通的字"
    );
    assert_eq!(
        words("\"a b\"c"),
        ["a b", "c"],
        "第 4 条：右引号后面紧跟着的是下一个词"
    );
    assert_eq!(words("   "), Vec::<&str>::new());
}

#[test]
fn the_rest_drops_its_quotes_only_when_it_is_one_quoted_word() {
    assert_eq!(rest_word("  \"naming and tests\" "), "naming and tests");
    assert_eq!(rest_word("naming and tests"), "naming and tests");
    assert_eq!(
        rest_word("\"a b\" and more"),
        "\"a b\" and more",
        "几个词照原文"
    );
    assert_eq!(rest_word(" one "), "one");
}

#[test]
fn indexed_arguments_follow_the_quotes_and_the_whole_keeps_them() {
    assert_eq!(
        expand("[$ARGUMENTS[0]] [$ARGUMENTS[1]]", "\"San Francisco\" CA"),
        "[San Francisco] [CA]"
    );
    assert_eq!(
        expand("All: $ARGUMENTS", "\"San Francisco\" CA"),
        "All: \"San Francisco\" CA",
        "整段照原文，引号照留"
    );
}
