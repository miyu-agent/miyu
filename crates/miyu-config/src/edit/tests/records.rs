//! 改一组字段的列表（施工 PC-2，设计 53 第二节第 3、5 条）：整条换。原来写成表的数组的照原来每一块的位置和上面的注释换；没写的
//! 新开几块，跟在所属的那张表（连它的子表）后面、别的表前面；换成空的写成 `<名字> = []`，落在所属的那张表里；行内的原地换；
//! 别的字节不动。
//!
//! 留不住的：去掉的那几块（换成更少的、空的、删掉）连同它们上面的注释一起没了；一块里字段照字母先后写，不照原来、不照清单的先后。

use crate::edit::{Change, apply};
use crate::secret::Reference;
use crate::value::Value;

fn account(name: &str, key: &str) -> Value {
    Value::Record(
        [
            ("name".to_string(), Value::Text(name.to_string().into())),
            (
                "key".to_string(),
                Value::Secret(Reference::Secret(key.to_string())),
            ),
        ]
        .into_iter()
        .collect(),
    )
}

const BLOCKS: &str = "# 生图\n[draw]\nsize = 512 # 边长\n\n# 第一个账号\n[[draw.accounts]]\nname = \"Old\"\nkey = { secret = \"old\" }\n\n# 第二个账号\n[[draw.accounts]]\nname = \"Old2\"\nkey = { secret = \"old2\" }\n\n[ui]\nlanguage = \"zh\"\n";

fn set(text: &str, value: &Value) -> String {
    apply(text, Change::Set("draw.accounts", value)).expect("放得进")
}

#[test]
fn table_blocks_are_replaced_where_they_were() {
    let two = Value::List(vec![account("Home", "home"), account("Work", "work")].into());
    assert_eq!(
        set(BLOCKS, &two),
        "# 生图\n[draw]\nsize = 512 # 边长\n\n# 第一个账号\n[[draw.accounts]]\nkey = { secret = \"home\" }\nname = \"Home\"\n\n# 第二个账号\n[[draw.accounts]]\nkey = { secret = \"work\" }\nname = \"Work\"\n\n[ui]\nlanguage = \"zh\"\n",
        "每一块上面的注释、块之间的空行照原来的"
    );
    let one = Value::List(vec![account("Solo", "solo")].into());
    assert_eq!(
        set(BLOCKS, &one),
        "# 生图\n[draw]\nsize = 512 # 边长\n\n# 第一个账号\n[[draw.accounts]]\nkey = { secret = \"solo\" }\nname = \"Solo\"\n\n[ui]\nlanguage = \"zh\"\n",
        "少了的那一块连同它上面的注释去掉"
    );
    let three = Value::List(vec![account("A", "a"), account("B", "b"), account("C", "c")].into());
    assert_eq!(
        set(BLOCKS, &three),
        "# 生图\n[draw]\nsize = 512 # 边长\n\n# 第一个账号\n[[draw.accounts]]\nkey = { secret = \"a\" }\nname = \"A\"\n\n# 第二个账号\n[[draw.accounts]]\nkey = { secret = \"b\" }\nname = \"B\"\n\n[[draw.accounts]]\nkey = { secret = \"c\" }\nname = \"C\"\n\n[ui]\nlanguage = \"zh\"\n",
        "多出来的接在原来最后一块后面、[ui] 前面"
    );
}

/// 原来的块不在所属的表后面（在别的表后面）：换完照旧在那里，不挪到 `[draw]` 后面。
#[test]
fn blocks_stay_where_they_were_written() {
    let far = "[draw]\nsize = 512\n\n[ui]\nlanguage = \"zh\"\n\n[[draw.accounts]]\nname = \"Old\"\nkey = { secret = \"old\" }\n";
    let one = Value::List(vec![account("Solo", "solo")].into());
    assert_eq!(
        set(far, &one),
        "[draw]\nsize = 512\n\n[ui]\nlanguage = \"zh\"\n\n[[draw.accounts]]\nkey = { secret = \"solo\" }\nname = \"Solo\"\n"
    );
}

#[test]
fn empty_unset_missing_and_inline() {
    let empty = Value::List(Vec::new().into());
    assert_eq!(
        set(BLOCKS, &empty),
        "# 生图\n[draw]\nsize = 512 # 边长\naccounts = []\n\n[ui]\nlanguage = \"zh\"\n",
        "空的写在 [draw] 里，原来的几块去掉"
    );
    assert_eq!(
        apply(BLOCKS, Change::Unset("draw.accounts")).expect("删得掉"),
        "# 生图\n[draw]\nsize = 512 # 边长\n\n[ui]\nlanguage = \"zh\"\n"
    );
    let two = Value::List(vec![account("Home", "home"), account("Work", "work")].into());
    let missing = "[draw]\nsize = 512\n\n[draw.style]\nmode = \"a\"\n\n[ui]\nlanguage = \"zh\"\n";
    assert_eq!(
        set(missing, &two),
        "[draw]\nsize = 512\n\n[draw.style]\nmode = \"a\"\n\n[[draw.accounts]]\nkey = { secret = \"home\" }\nname = \"Home\"\n\n[[draw.accounts]]\nkey = { secret = \"work\" }\nname = \"Work\"\n\n[ui]\nlanguage = \"zh\"\n",
        "新开的几块跟在 [draw]（连它的子表）后面、[ui] 前面"
    );
    assert_eq!(
        set(missing, &empty),
        "[draw]\nsize = 512\naccounts = []\n\n[draw.style]\nmode = \"a\"\n\n[ui]\nlanguage = \"zh\"\n",
        "空的落在 [draw] 里、子表的表头前面"
    );
    assert_eq!(
        set(
            "[[draw.accounts]]\nname = \"Old\"\nkey = { secret = \"old\" }\n\n[ui]\nlanguage = \"zh\"\n",
            &empty
        ),
        "[draw]\naccounts = []\n\n[ui]\nlanguage = \"zh\"\n",
        "原来只有几块、没有 [draw] 表头的，开一个"
    );
    assert_eq!(
        set("", &two),
        "[[draw.accounts]]\nkey = { secret = \"home\" }\nname = \"Home\"\n\n[[draw.accounts]]\nkey = { secret = \"work\" }\nname = \"Work\"\n"
    );
    let inline =
        "[draw]\naccounts = [{ name = \"Old\", key = { secret = \"old\" } }] # 账号\nsize = 512\n";
    assert_eq!(
        set(inline, &two),
        "[draw]\naccounts = [{ key = { secret = \"home\" }, name = \"Home\" }, { key = { secret = \"work\" }, name = \"Work\" }] # 账号\nsize = 512\n",
        "行内的原地换，行尾注释留着"
    );
}
