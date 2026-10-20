//! 拆命令（施工 5-17）：边界、引号、反斜杠、替换、注释。

use super::*;

/// 每条管道里每个简单命令的词。
fn words(text: &str) -> Vec<Vec<Vec<String>>> {
    parse(text)
        .pipelines
        .iter()
        .map(|pipeline| {
            pipeline
                .iter()
                .map(|simple| simple.words.iter().map(|word| word.text.clone()).collect())
                .collect()
        })
        .collect()
}

fn one(text: &str) -> Vec<String> {
    vec![text.to_string()]
}

#[test]
fn boundaries_split_pipelines_and_commands() {
    assert_eq!(words("a || b"), [[one("a")], [one("b")]]);
    assert_eq!(words("a && b"), [[one("a")], [one("b")]]);
    assert_eq!(words("a |& b"), [vec![one("a"), one("b")]]);
    assert_eq!(
        words("a | b; c"),
        vec![vec![one("a"), one("b")], vec![one("c")]]
    );
    // 词中间的花括号不是边界。
    assert_eq!(words("a{b}c"), [[one("a{b}c")]]);
    // 注释到行尾；词中间的 `#` 是字。
    assert_eq!(words("ls # rm -rf ~"), [[one("ls")]]);
    assert_eq!(words("a#b"), [[one("a#b")]]);
}

#[test]
fn quotes_and_backslashes_are_taken_off() {
    assert_eq!(words("a\\ b"), [[one("a b")]]);
    assert_eq!(words("a\\\nb"), [[one("ab")]]);
    assert_eq!(words("'a b'\"c d\""), [[one("a bc d")]]);
    let word = |text: &str| parse(text).pipelines[0][0].words[0].clone();
    assert!(word("$X").unresolved);
    assert!(word("\"$X\"").unresolved);
    assert!(!word("'$X'").unresolved);
    assert!(!word("plain").unresolved);
}

#[test]
fn substitutions_are_read_again_inside() {
    let parsed = parse("echo $(rm -rf ~)");
    assert_eq!(parsed.inner, ["rm -rf ~"]);
    assert!(parsed.pipelines[0][0].words[1].unresolved);
    assert_eq!(parse("echo $(a $(b) c)").inner, ["a $(b) c"]);
    assert_eq!(parse("echo $(echo ')')").inner, ["echo ')'"]);
    assert_eq!(parse("echo \"$(x)\" `y`").inner, ["x", "y"]);
    assert_eq!(parse("bash <(curl z)").inner, ["curl z"]);
}

#[test]
fn redirects_are_written_targets() {
    let parsed = parse("echo x > /dev/sda 2>&1 < in");
    let simple = &parsed.pipelines[0][0];
    let writes: Vec<&str> = simple
        .writes
        .iter()
        .map(|word| word.text.as_str())
        .collect();
    assert_eq!(writes, ["/dev/sda", "1"]);
    let words: Vec<&str> = simple.words.iter().map(|word| word.text.as_str()).collect();
    assert_eq!(words, ["echo", "x"], "文件描述符、读进来的都不是参数");
}
