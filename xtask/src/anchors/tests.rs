//! 参考里的锚点。

use super::*;

fn pages(list: &[(&str, &str)]) -> BTreeMap<String, String> {
    list.iter()
        .map(|(name, text)| ((*name).to_string(), (*text).to_string()))
        .collect()
}

#[test]
fn anchors_are_worked_out_like_github() {
    let text = "# 连接：怎么连、握手、订阅\n\n## `subscribe`、`unsubscribe`\n\n## `account.setup_code`、`account.setup`\n\n## 你的头像\n\n## Your avatar\n\n## 你的头像\n";
    assert_eq!(
        headings(text),
        [
            "accountsetup_codeaccountsetup",
            "subscribeunsubscribe",
            "your-avatar",
            "你的头像",
            "你的头像-1",
            "连接怎么连握手订阅",
        ]
        .into_iter()
        .map(String::from)
        .collect::<BTreeSet<_>>()
    );
}

#[test]
fn a_hash_line_in_a_code_block_is_not_a_heading() {
    let text = "## 真的\n\n```sh\n# 不是标题\n```\n\n#不是标题\n####### 也不是\n";
    assert_eq!(headings(text), BTreeSet::from(["真的".to_string()]));
}

#[test]
fn a_link_to_a_missing_heading_is_named() {
    let found = problems(&pages(&[
        (
            "connection.md",
            "| [`subscribe`](#subscribeunsubscribe) | 订阅 |\n| [`hello`](#hello) | 握手 |\n\n## `hello`\n",
        ),
        (
            "pushes.md",
            "见[连接](connection.md#hello)、[没有的](connection.md#nope)。\n",
        ),
    ]));
    assert_eq!(
        found,
        [
            "docs/reference/connection.md：链到 connection.md#subscribeunsubscribe，那一页没有这个标题",
            "docs/reference/pushes.md：链到 connection.md#nope，那一页没有这个标题",
        ]
    );
}

#[test]
fn links_elsewhere_are_not_checked() {
    let found = problems(&pages(&[(
        "a.md",
        "[网址](https://example.com/x#y)、[蓝图](../blueprint/protocol.md#nope)、[整页](b.md)、[空的](#)\n",
    )]));
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn the_reference_in_the_repository_links_to_headings_that_exist() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("仓库根")
        .to_path_buf();
    assert_eq!(check(&root), Vec::<String>::new());
}
