//! 组表的读法（施工 BS-1 下）。

use super::*;

#[test]
fn groups_are_read_in_order_with_their_words() {
    let read = read(
        "[b]\nname = { en = \"B\" }\n\n[a]\nname = { en = \"A\", zh = \"甲\" }\nsummary = { en = \"First\" }\n",
    )
    .expect("读得成");
    let ids: Vec<&str> = read.iter().map(|group| group.id.as_str()).collect();
    assert_eq!(ids, ["b", "a"]);
    assert_eq!(read[1].name.get("zh").map(String::as_str), Some("甲"));
    assert!(read[0].summary.is_empty());
    assert!(read[0].order.is_empty(), "没写的是空的");
}

/// 组里的包照用途的先后（施工 PG-1）：`order` 是编号的一串，照写的先后。
#[test]
fn a_group_may_list_its_order() {
    let read = read("[b]\nname = { en = \"B\" }\norder = [\"read\", \"write\"]\n").expect("读得成");
    assert_eq!(read[0].order, ["read", "write"]);
}

#[test]
fn a_wrong_group_says_where() {
    for (text, said) in [
        ("[\"B!\"]\nname = { en = \"B\" }\n", "B!: not a valid id"),
        ("[b]\nsummary = { en = \"B\" }\n", "b.name: missing"),
        (
            "[b]\nname = { en = \"B\" }\nicon = \"x\"\n",
            "b.icon: unknown key",
        ),
        ("b = 1\n", "b: not a table"),
        (
            "[b]\nname = { en = \"B\" }\norder = \"read\"\n",
            "b.order: not a list of ids",
        ),
        (
            "[b]\nname = { en = \"B\" }\norder = [1]\n",
            "b.order: not a list of ids",
        ),
    ] {
        assert_eq!(read(text), Err(said.to_string()), "{text}");
    }
}
