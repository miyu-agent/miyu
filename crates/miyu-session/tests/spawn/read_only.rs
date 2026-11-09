//! 子代理写了只读（施工 SA-1，2026-11-05 项目主人定）：父会话能写的，子会话的权限照抄、只读开关打开；没写的照抄父会话；
//! 父会话只读的照旧只读（[`super::the_child_copies_the_parent_and_gets_the_task_from_it`]）。

use super::*;

/// 能写的父会话（工作区那一级、只读关着）派一次子代理，参数另加 `extra`；交回造的子会话。
async fn child_of_writable(extra: serde_json::Value) -> Child {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let mut args = serde_json::json!({"description": "查导出", "prompt": "List the exports."});
    for (key, value) in extra.as_object().expect("是对象") {
        args[key] = value.clone();
    }
    let script = Script::new([
        calls(&[("subagent", args.to_string())]),
        Play::Says("派出去了。"),
    ]);
    let opening = Opening {
        permission: Permission {
            level: Level::Full,
            read_only: false,
        },
        ..Opening::default()
    };
    let lines = Lines {
        sessions: Some(Arc::clone(&table) as Arc<dyn SessionPort>),
        ..Lines::default()
    };
    let handle = home
        .create_full(&script, &basesystem(&home), opening, lines)
        .await;
    one_turn(&home, &handle, 1).await;
    let [child] = table.made().try_into().expect("造了一个子会话");
    child
}

#[tokio::test]
async fn a_read_only_child_gets_the_parents_level_with_read_only_on() {
    let plain = child_of_writable(serde_json::json!({})).await;
    assert_eq!(
        plain.permission,
        Permission {
            level: Level::Full,
            read_only: false
        },
        "没写的照抄父会话"
    );
    let read_only = child_of_writable(serde_json::json!({"read_only": true})).await;
    assert_eq!(
        read_only.permission,
        Permission {
            level: Level::Full,
            read_only: true
        },
        "写了只读的打开只读开关，常用的那一级照抄"
    );
    let off = child_of_writable(serde_json::json!({"read_only": false})).await;
    assert!(!off.permission.read_only, "写 false 的同没写");
}
