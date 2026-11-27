//! `edit` 认的每一种写法，和标准写法一样照参数格式修正（施工 ED-3）：顶层直接写一处、`edits` 写成一个对象、`path`、
//! `oldText`、`replaceAll` 这些别家的字段名，写成字符串的布尔照样还原。走的是内核那一条：先照参数格式修正（[`repair`]），
//! 再交给工具。读不懂的、还原不了的照原样报原文的错：下面几句是改之前逐字记下的。

use serde_json::{Value, json};

use miyu_kernel::block::Block;
use miyu_kernel::tool::repair;
use miyu_tool::{Done, Seen};

use crate::support::{Site, tool};

/// 交回的字。
fn text(done: &Done) -> String {
    done.blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

/// 照内核的样子调一次 `edit`：参数先照 `edit` 的参数格式修正，没读过（施工 ED-1 起照改）。
async fn edit(site: &Site, args: &Value) -> Done {
    let spec = tool("edit").spec().clone();
    let repaired = repair(&spec.parameters, &args.to_string()).expect("是一个对象");
    let args: Value = serde_json::from_str(&repaired).expect("修正出来的是 JSON");
    site.done_seen("work", "edit", args, Seen::new()).await
}

#[tokio::test]
async fn every_shape_takes_a_boolean_written_as_text() {
    let site = Site::new();
    for args in [
        json!({"file_path": "a.txt", "old_string": "x", "new_string": "y", "replace_all": "true"}),
        json!({"path": "a.txt", "oldText": "x", "newText": "y", "replaceAll": "true"}),
        json!({"filePath": "a.txt", "edits": {"old_string": "x", "new_string": "y", "replace_all": "True"}}),
        json!({"file_path": "a.txt", "edits": [{"oldString": "x", "newString": "y", "replaceAll": "true"}]}),
        // `edits` 外面多写的顶层 `replace_all` 照旧不算数，写成字符串的也不报错。
        json!({"file_path": "a.txt", "edits": [{"old_string": "x", "new_string": "y", "replace_all": true}], "replace_all": "false"}),
    ] {
        site.file("work/a.txt", b"x x\n");
        let done = edit(&site, &args).await;
        assert!(!done.error, "{args}：{}", text(&done));
        assert_eq!(
            std::fs::read(site.0.join("work/a.txt")).expect("读得出"),
            b"y y\n",
            "{args}"
        );
    }
}

#[tokio::test]
async fn what_cannot_be_read_says_what_it_said_before() {
    let site = Site::new();
    site.file("work/a.txt", b"x x\n");
    for (args, error) in [
        (
            json!({"file_path": "a.txt", "old_string": "x", "new_string": "y", "replace_all": "yes"}),
            "Bad arguments: invalid type: string \\u0022yes\\u0022, expected a boolean at line 1 column 74.\n",
        ),
        (
            json!({"file_path": "a.txt", "path": "a.txt", "old_string": "x", "new_string": "y"}),
            "Bad arguments: duplicate field `file_path` at line 1 column 61.\n",
        ),
        (
            json!({"old_string": "x", "new_string": "y"}),
            "Bad arguments: missing field `file_path` at line 1 column 35.\n",
        ),
    ] {
        let done = edit(&site, &args).await;
        assert!(done.error, "{args}");
        assert_eq!(text(&done), error, "{args}");
    }
    assert_eq!(
        std::fs::read(site.0.join("work/a.txt")).expect("读得出"),
        b"x x\n"
    );
}
