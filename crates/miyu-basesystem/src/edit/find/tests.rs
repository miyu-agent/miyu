//! 找位置：精确的、CRLF 照 LF 写的、宽松的几种；精确的不唯一不往宽松找；换回原文里的位置，宽松多出来的不算进去；
//! `replace_all`；不唯一的给行号；对不上的给最接近的几行。

use super::*;

/// 对上的那一段原文。
fn at<'a>(text: &'a str, old: &str) -> Vec<&'a str> {
    match find(text, old, false) {
        Found::At(ranges) => ranges.into_iter().map(|range| &text[range]).collect(),
        other => panic!("应该对得上：{other:?}"),
    }
}

#[test]
fn exact_text_is_found_where_it_is() {
    let text = "fn a() {}\nfn b() {}\n";
    assert_eq!(at(text, "fn b"), ["fn b"]);
    match find(text, "fn b", false) {
        Found::At(ranges) => assert_eq!((ranges.len(), ranges[0].clone()), (1, 10..14)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn lf_matches_a_crlf_file_and_takes_its_line_break_along() {
    let text = "one\r\ntwo\r\nthree\r\n";
    assert_eq!(
        at(text, "two\n"),
        ["two\r\n"],
        "对上的一段连着 CRLF 两个字节"
    );
    assert_eq!(at(text, "one\ntwo"), ["one\r\ntwo"]);
    assert_eq!(at(text, "two"), ["two"], "没带换行的不带走 CR");
}

#[test]
fn loose_matching_forgives_trailing_blanks_quotes_dashes_and_wide_forms() {
    // 文件里行尾有空白：她写的没有。
    assert_eq!(
        at("let a = 1;   \nlet b;\n", "let a = 1;\nlet b;"),
        ["let a = 1;   \nlet b;"]
    );
    // 弯引号、破折号、全角空格、全角字母。
    assert_eq!(
        at("say \u{201C}hi\u{201D}\n", "say \"hi\""),
        ["say \u{201C}hi\u{201D}"]
    );
    assert_eq!(at("a \u{2014} b\n", "a - b"), ["a \u{2014} b"]);
    assert_eq!(
        at("x\u{3000}=\u{3000}1\n", "x = 1"),
        ["x\u{3000}=\u{3000}1"]
    );
    assert_eq!(
        at("\u{FF41}\u{FF42}\u{FF43}\n", "abc"),
        ["\u{FF41}\u{FF42}\u{FF43}"]
    );
    // 她写的行尾有空白、文件里没有，也对得上。
    assert_eq!(at("fn a() {}\n", "fn a() {}  "), ["fn a() {}"]);
}

#[test]
fn the_loose_match_takes_only_what_it_matched() {
    // 对上的是这一行里的一段：行尾的空白不在这一段里。
    let text = "value = 1  \nnext\n";
    assert_eq!(at(text, "value = 1"), ["value = 1"]);
}

#[test]
fn an_exact_match_that_is_not_unique_does_not_fall_back_to_loose() {
    let text = "x = 1\ny = 2\nx = 1\n";
    assert_eq!(
        find(text, "x = 1", false),
        Found::Many {
            count: 2,
            lines: vec![1, 3]
        }
    );
    // 精确的对上两个，宽松的能对上三个（第三行是弯引号）：照精确那一层说，不往下找。
    let quoted = "say \"hi\"\nsay \"hi\"\nsay \u{201C}hi\u{201D}\n";
    assert_eq!(
        find(quoted, "say \"hi\"", false),
        Found::Many {
            count: 2,
            lines: vec![1, 2]
        }
    );
    // 重叠的也算不唯一。
    assert_eq!(
        find("aaa", "aa", false),
        Found::Many {
            count: 2,
            lines: vec![1, 1]
        }
    );
}

#[test]
fn replace_all_takes_every_match_without_overlap() {
    let text = "a.b a.b\na.b\n";
    match find(text, "a.b", true) {
        Found::At(ranges) => assert_eq!(ranges, [0..3, 4..7, 8..11]),
        other => panic!("{other:?}"),
    }
    match find("aaa", "aa", true) {
        Found::At(ranges) => assert_eq!((ranges.len(), ranges[0].clone()), (1, 0..2), "不重叠的"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn many_matches_list_at_most_ten_lines() {
    let text = "same\n".repeat(12);
    match find(&text, "same", false) {
        Found::Many { count, lines } => {
            assert_eq!(count, 12);
            assert_eq!(lines, (1..=10).collect::<Vec<_>>());
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_miss_points_at_the_closest_lines() {
    let text = "fn main() {\n    let total = count(items);\n    println!(\"{total}\");\n}\n";
    let old = "    let totals = count(item);\n    println!(\"{totals}\");\n";
    match find(text, old, false) {
        Found::Missing(Some(closest)) => {
            assert_eq!((closest.from, closest.to), (2, 3));
            assert_eq!(
                closest.text,
                "2\t    let total = count(items);\n3\t    println!(\"{total}\");\n"
            );
        }
        other => panic!("{other:?}"),
    }
    // 第一行是空的：从第一行不空的那一行比，往回算上空的那几行。
    match find(text, "\n    let totals = count(item);\n", false) {
        Found::Missing(Some(closest)) => assert_eq!((closest.from, closest.to), (1, 2)),
        other => panic!("{other:?}"),
    }
    // 没有像的：只说没找到。
    assert_eq!(find(text, "zzzz qqqq", false), Found::Missing(None));
    assert_eq!(
        find(text, "   \n  ", false),
        Found::Missing(None),
        "全是空白的，没得比"
    );
}

#[test]
fn how_alike_two_lines_are() {
    assert!((alike("abc", "abc") - 1.0).abs() < f64::EPSILON);
    assert!((alike("abcd", "abce") - 2.0 / 3.0).abs() < 1e-9);
    assert!(alike("ab", "") < f64::EPSILON);
    assert!(alike("a", "b") < f64::EPSILON);
}

/// 空白兜底（施工 E-D）：前两层都对不上，不管行首空白才对上唯一一处。
fn indented(text: &str, old: &str) -> (String, String) {
    match find(text, old, false) {
        Found::Indented { range, indent } => (text[range].to_string(), indent),
        other => panic!("应该不管缩进对上：{other:?}"),
    }
}

#[test]
fn a_different_indentation_matches_from_the_first_word_of_the_line() {
    let text = "fn a() {\n    if x {\n        go();\n    }\n}\n";
    let (found, indent) = indented(text, "  if x {\n    go();\n  }");
    assert_eq!(
        found, "if x {\n        go();\n    }",
        "从这一行头一个字起，别的照原文"
    );
    assert_eq!(indent, "    ");
    // 制表符写的也认。
    let (found, indent) = indented("\tlet a = 1;\n\tlet b = 2;\n", "let a = 1;\nlet b = 2;");
    assert_eq!(
        (found.as_str(), indent.as_str()),
        ("let a = 1;\n\tlet b = 2;", "\t")
    );
}

#[test]
fn the_indentation_fallback_needs_one_match_from_a_line_start() {
    // 两处都只差缩进：不唯一，照旧报找不到。
    let text = "  a();\n    b();\n      a();\n    b();\n";
    assert!(matches!(find(text, "a();\nb();", false), Found::Missing(_)));
    // 不从一行的开头起的不算：`x = go()` 里的 `go()`。
    assert!(matches!(
        find("let x = go();\nstop();\n", " go();\n stop();", false),
        Found::Missing(_)
    ));
    // 全部替换的不走这一层。
    assert!(matches!(
        find("  a();\n", "    a();", true),
        Found::Missing(_)
    ));
    // 第一行是空的不找：对上的会从换行起，缩进对不齐。
    assert!(matches!(
        find("\n    a();\n", "\n  a();", false),
        Found::Missing(_)
    ));
}

#[test]
fn the_new_text_takes_the_files_indentation() {
    // 她每级两格、最外一层两格；原文每级四格、最外一层四格：几级照她的数，每级换成原文的写法。
    let matched = "if x {\n        go();\n    }";
    assert_eq!(
        reindent(
            "  if y {\n    stop();\n      deeper();\n  }",
            "  if x {\n    go();\n  }",
            matched,
            "    "
        ),
        "if y {\n        stop();\n            deeper();\n    }"
    );
    // 原文用制表符。
    assert_eq!(
        reindent(
            "a {\n  b();\n}",
            "a {\n  c();\n}",
            "a {\n\t\tc();\n\t}",
            "\t"
        ),
        "a {\n\t\tb();\n\t}"
    );
    // 空行照旧；看不出一级多宽的（只有一行）照原样接在原文那份后面。
    assert_eq!(
        reindent("a();\n\n  b();\n", "a();", "a();", "\t"),
        "a();\n\n\t  b();\n"
    );
    assert_eq!(
        reindent(" c();", "  a();", "a();", "\t"),
        "c();",
        "第一行去掉开头的空白"
    );
}
