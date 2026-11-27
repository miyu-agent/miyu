//! 编辑、写入不是标准写法的参数（施工 ED-3）：顶层直接写一处、`edits` 写成一个对象、`path`、`oldText` 这些别家的字段名，
//! 标题、照参数估的行数和标准写法的一样；头要的标准写法的参数另给 `standard_args`，本来就是标准写法的不给。`args` 照旧是
//! 参数原文。

use miyu_kernel::testkit::{Line, Play};
use serde_json::{Value, json};

use crate::support::*;

/// 她调一次 `name`，参数是 `args`，成了：交回那一步。
fn one_step(name: &str, args: &Value) -> Value {
    let mut stage = stage();
    stage.model([
        Line::calls("", &[(name, args.to_string().as_str())]),
        Line::says("Done."),
    ]);
    stage.tools([Play::done("ok")]);
    stage.say("go");
    let entries = same(&stage);
    json(of_kind(&entries, "tool")[0])
}

#[test]
fn an_edit_in_any_shape_has_its_title_and_lines() {
    let standard =
        json!({"file_path": "a.txt", "edits": [{"old_string": "a\n", "new_string": "b\nc\n"}]});
    for args in [
        json!({"path": "a.txt", "oldText": "a\n", "newText": "b\nc\n", "replaceAll": "true"}),
        json!({"file_path": "a.txt", "old_string": "a\n", "new_string": "b\nc\n"}),
        json!({"filePath": "a.txt", "edits": {"oldString": "a\n", "newString": "b\nc\n"}}),
    ] {
        let tool = one_step("edit", &args);
        assert_eq!(tool["args"], args.to_string(), "参数原文照旧");
        assert_eq!(tool["title"]["object"], "a.txt", "{args}");
        assert_eq!(
            tool["diff"],
            json!({"added": 2, "removed": 1, "estimated": true}),
            "{args}"
        );
        let shaped: Value =
            serde_json::from_str(tool["standard_args"].as_str().unwrap_or_default())
                .unwrap_or_else(|_| panic!("{args} 有标准写法的参数：{tool}"));
        assert_eq!(shaped["file_path"], standard["file_path"], "{args}");
        assert_eq!(shaped["edits"][0]["old_string"], "a\n", "{args}");
        assert_eq!(shaped["edits"][0]["new_string"], "b\nc\n", "{args}");
    }
    let tool = one_step("edit", &standard);
    assert!(tool.get("standard_args").is_none(), "{tool}");
}

#[test]
fn a_write_with_a_path_alias_has_its_title_and_lines() {
    let tool = one_step("write", &json!({"path": "b.txt", "content": "x\ny\n"}));
    assert_eq!(tool["title"]["object"], "b.txt");
    assert_eq!(
        tool["diff"],
        json!({"added": 2, "removed": 0, "estimated": true})
    );
    let shaped: Value = serde_json::from_str(tool["standard_args"].as_str().unwrap_or_default())
        .unwrap_or_else(|_| panic!("有标准写法的参数：{tool}"));
    assert_eq!(shaped, json!({"file_path": "b.txt", "content": "x\ny\n"}));
}

#[test]
fn other_tools_keep_their_own_names() {
    // 不算编辑、写入的不规整（`grep`、`show` 的 `path` 是它们自己的标准名）。
    let tool = one_step("read", &json!({"path": "src/a.rs"}));
    assert!(tool.get("standard_args").is_none(), "{tool}");
}
