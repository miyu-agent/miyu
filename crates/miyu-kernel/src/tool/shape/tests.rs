//! 读、写、改文件的参数规整成标准写法（施工 ED-3）：别名换成标准名、顶层写一处的收进 `edits`、`edits` 是对象的包成列表、
//! 有 `edits` 时顶层的 `replace_all` 去掉；值原样；本来就是标准写法的、读不懂的照原文。

use serde_json::{Value, json};

use super::{standard, standard_value};

/// 规整 `args`：交回规整过的，没改的是 `None`。
fn shaped(args: &Value) -> Option<Value> {
    let mut value = args.clone();
    standard_value(&mut value).then_some(value)
}

#[test]
fn every_shape_comes_out_standard() {
    let standard = json!({"file_path": "a", "edits": [{"old_string": "x", "new_string": "y", "replace_all": "true"}]});
    for args in [
        json!({"file_path": "a", "old_string": "x", "new_string": "y", "replace_all": "true"}),
        json!({"path": "a", "oldText": "x", "newText": "y", "replaceAll": "true"}),
        json!({"filePath": "a", "oldString": "x", "newString": "y", "replaceAll": "true"}),
        json!({"file_path": "a", "edits": {"old_string": "x", "new_string": "y", "replace_all": "true"}}),
        json!({"path": "a", "edits": [{"oldString": "x", "newText": "y", "replaceAll": "true"}]}),
        json!({"file_path": "a", "edits": [{"old_string": "x", "new_string": "y", "replace_all": "true"}], "replaceAll": "false"}),
        json!({"file_path": "a", "edits": null, "old_string": "x", "new_string": "y", "replace_all": "true"}),
    ] {
        assert_eq!(shaped(&args).as_ref(), Some(&standard), "{args}");
    }
}

#[test]
fn what_is_standard_or_cannot_be_read_stays_as_it_is() {
    for args in [
        json!({"file_path": "a", "edits": [{"old_string": "x", "new_string": "y"}]}),
        json!({"file_path": "a", "content": "x"}),
        json!({"file_path": "a"}),
        // 同一格写了两个名字。
        json!({"file_path": "a", "path": "b", "old_string": "x", "new_string": "y"}),
        json!({"file_path": "a", "old_string": "x", "oldText": "z", "new_string": "y"}),
        json!({"file_path": "a", "edits": [{"old_string": "x", "oldString": "z", "new_string": "y"}]}),
        json!({"file_path": "a", "edits": {"replace_all": true, "replaceAll": false}}),
        json!("not an object"),
        json!([1, 2]),
    ] {
        assert_eq!(shaped(&args), None, "{args}");
        let text = args.to_string();
        assert!(
            matches!(standard(&text), std::borrow::Cow::Borrowed(_)),
            "{args}"
        );
    }
    assert!(matches!(
        standard("{oops"),
        std::borrow::Cow::Borrowed("{oops")
    ));
}

#[test]
fn half_a_change_only_takes_the_standard_names() {
    // 缺 `new_string` 的不收进 `edits`：工具照旧说没给要改的。
    assert_eq!(
        shaped(&json!({"path": "a", "oldText": "x"})),
        Some(json!({"file_path": "a", "old_string": "x"}))
    );
    // `edits` 写成字符串、修正还原不了的：只换路径，交给工具照原样报错。
    assert_eq!(
        shaped(&json!({"path": "a", "edits": "oops"})),
        Some(json!({"file_path": "a", "edits": "oops"}))
    );
    // 不是对象的那几项照原样。
    assert_eq!(
        shaped(&json!({"file_path": "a", "edits": [1, {"oldText": "x", "new_string": "y"}]})),
        Some(json!({"file_path": "a", "edits": [1, {"old_string": "x", "new_string": "y"}]}))
    );
}

#[test]
fn values_are_kept_byte_for_byte() {
    let old = "  tab\there \u{201C}quote\u{201D}\r\n\\u0041 \"x\"";
    let args = json!({"path": "a", "oldText": old, "newText": "", "replaceAll": 1});
    let text = standard(&args.to_string()).into_owned();
    let value: Value = serde_json::from_str(&text).expect("JSON");
    assert_eq!(value["edits"][0]["old_string"], old);
    assert_eq!(value["edits"][0]["new_string"], "");
    assert_eq!(value["edits"][0]["replace_all"], 1, "不是这里还原的");
}
