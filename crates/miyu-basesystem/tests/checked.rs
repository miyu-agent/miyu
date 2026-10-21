//! 工具只碰判过的位置（施工 5-18，`docs/blueprint/tools/interface.md`「判过的位置」）：调用带着权限策略判过的真实位置，工具
//! 换出来的是其中一个才碰；不在里面的（判完以后路径上的链接被换过）一律不碰，说路径在判过以后变了。往下走目录的只核搜的
//! 那个目录。附件（`send_message`）的在 `send_message/attachments.rs`。

use serde_json::json;

use miyu_kernel::block::Block;
use miyu_tool::Done;

use crate::support::Site;

fn text(done: &Done) -> String {
    done.blocks
        .iter()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect()
}

/// 场地里 `work/a.txt` 写着 `a`。
fn site() -> Site {
    let site = Site::new();
    site.file("work/a.txt", b"a");
    site
}

/// 判过的不是工具要碰的：每一件都不碰，说变了。
#[tokio::test]
async fn every_file_tool_refuses_a_position_that_was_not_judged() {
    let site = site();
    let elsewhere = vec![site.real("work").join("elsewhere")];
    let calls = [
        ("read", json!({"file_path": "a.txt"})),
        ("write", json!({"file_path": "a.txt", "content": "b"})),
        (
            "edit",
            json!({"file_path": "a.txt", "old_string": "a", "new_string": "b"}),
        ),
        ("trash", json!({"file_path": "a.txt"})),
        ("show", json!({"path": "a.txt"})),
        ("glob", json!({"pattern": "*.txt", "path": "."})),
        ("grep", json!({"pattern": "a", "path": "."})),
    ];
    for (name, args) in calls {
        let done = site
            .done_checked(name, args, None, Some(elsewhere.clone()))
            .await;
        assert!(done.error, "{name}：{done:?}");
        assert!(
            text(&done).contains("changed after the permission check"),
            "{name}：{}",
            text(&done)
        );
    }
    assert_eq!(
        std::fs::read_to_string(site.0.join("work/a.txt")).expect("还在"),
        "a",
        "没改、没删"
    );
}

/// 判过的就是工具要碰的：照常读、写、找、搜，新建的照上级判过的那一个。
#[tokio::test]
async fn a_judged_position_is_used_as_before() {
    let site = site();
    let a = vec![site.real("work/a.txt")];
    let read = site
        .done_checked("read", json!({"file_path": "a.txt"}), None, Some(a.clone()))
        .await;
    assert!(!read.error && text(&read).contains('a'), "{read:?}");
    let new = vec![site.real("work").join("new.txt")];
    let write = site
        .done_checked(
            "write",
            json!({"file_path": "new.txt", "content": "n"}),
            None,
            Some(new),
        )
        .await;
    assert!(!write.error, "{write:?}");
    assert_eq!(
        std::fs::read_to_string(site.0.join("work/new.txt")).expect("写了"),
        "n"
    );
    let work = vec![site.real("work")];
    for (name, args) in [
        ("glob", json!({"pattern": "*.txt", "path": "."})),
        ("grep", json!({"pattern": "a", "path": "."})),
    ] {
        let done = site
            .done_checked(name, args, None, Some(work.clone()))
            .await;
        assert!(!done.error, "{name}：{done:?}");
        assert!(text(&done).contains("a.txt"), "{name}：{}", text(&done));
    }
}
