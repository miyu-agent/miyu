//! 提示词的参数（施工 X-39）：照空白切开照先后对上，最后一个拿剩下的整段；只有一个的拿全部；没声明的接在末尾；缺必填的说缺哪几个。
//! 菜单那一项：说明照 `title`、再照 `description`，参数照必填、可选拼。

use std::time::Duration;

use miyu_mcp::Prompt;

use super::*;

fn argument(name: &str, required: bool) -> Argument {
    Argument {
        name: name.to_string(),
        description: None,
        required,
    }
}

fn given(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
        .collect()
}

#[test]
fn words_go_to_arguments_in_order_and_the_last_takes_the_rest() {
    let two = [argument("pr", true), argument("focus", false)];
    assert_eq!(
        arguments(&two, "  456   naming and  tests "),
        Ok((
            given(&[("pr", "456"), ("focus", "naming and  tests")]),
            None
        ))
    );
    assert_eq!(
        arguments(&two, "456"),
        Ok((given(&[("pr", "456")]), None)),
        "可选的没给不算缺"
    );
    let one = [argument("code", true)];
    assert_eq!(
        arguments(&one, "x = 1\ny = 2"),
        Ok((given(&[("code", "x = 1\ny = 2")]), None)),
        "只有一个的拿全部"
    );
}

/// 带引号的（施工 X-40）：成对的双引号中间是一个参数；最后一个只剩一个带引号的词去掉引号，剩几个词照原文；空的引号当没给。
#[test]
fn quoted_words_go_to_one_argument_and_empty_quotes_are_not_given() {
    let city = [argument("city", true), argument("state", false)];
    assert_eq!(
        arguments(&city, "\"San Francisco\" CA"),
        Ok((given(&[("city", "San Francisco"), ("state", "CA")]), None))
    );
    assert_eq!(
        arguments(&city, "Tokyo \"Kanto region\""),
        Ok((given(&[("city", "Tokyo"), ("state", "Kanto region")]), None)),
        "最后一个只剩一个带引号的词，去掉引号"
    );
    assert_eq!(
        arguments(&city, "Tokyo \"Kanto\" region"),
        Ok((
            given(&[("city", "Tokyo"), ("state", "\"Kanto\" region")]),
            None
        )),
        "剩几个词照原文"
    );
    assert_eq!(
        arguments(&city, "\"\" CA"),
        Err(vec!["city".to_string()]),
        "空的引号当没给，必填的照缺"
    );
    let one = [argument("code", true)];
    assert_eq!(
        arguments(&one, " \"fn main() {}\" "),
        Ok((given(&[("code", "fn main() {}")]), None)),
        "只有一个的拿全部，正好一个带引号的词去掉引号"
    );
}

#[test]
fn without_declared_arguments_the_words_come_after() {
    assert_eq!(
        arguments(&[], "  extra words "),
        Ok((BTreeMap::new(), Some("extra words".to_string())))
    );
    assert_eq!(arguments(&[], "   "), Ok((BTreeMap::new(), None)));
}

#[test]
fn missing_required_arguments_are_named() {
    let both = [argument("pr", true), argument("repo", true)];
    assert_eq!(
        arguments(&both, ""),
        Err(vec!["pr".to_string(), "repo".to_string()])
    );
    assert_eq!(arguments(&both, "456"), Err(vec!["repo".to_string()]));
}

#[test]
fn a_catalog_item_says_what_follows_the_name() {
    let offered = Offered {
        command: "github:pr_review".to_string(),
        prompt: Prompt {
            name: "pr_review".to_string(),
            title: None,
            description: Some("Review a pull request.".to_string()),
            arguments: vec![argument("pr", true), argument("focus", false)],
        },
        wait: Duration::from_secs(1),
    };
    assert_eq!(
        listed(&offered),
        json!({"name": "github:pr_review", "aliases": [], "summary": "Review a pull request.", "argument": "<pr> [focus]"})
    );
}
