//! 审计复现（漏洞排查 2026-10-22）：数据根只在权限检查那一步挡（`miyu-session` 的 `guard.rs` 照它那一次解析的真实位置判），
//! 文件工具执行时自己再解析一次路径（`write.rs`、`read.rs`、`edit.rs` 的 `resolve`），不再核对落没落在数据根里。两次解析之间
//! 路径上的链接变了，工具碰的就不是检查过的那一个（`11-权限与沙盒.md` 说「检查完、打开前」由逐层打开挡住，但逐层打开只管工具
//! 自己那一次解析之后）。
//!
//! 修法（施工 5-18）：判的和碰的是同一个。权限策略把判过的真实位置交给工具（`Call::checked`），工具换出来的不在里面不碰；
//! 不只数据根，落到工作区外、私密目录的也一样。这里直接调工具：判过的是工作区里的那一处，工具要碰的却落在数据根里，等于
//! 权限检查那一步看到的和执行时不是同一个位置。修之前这条是红的。

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

#[tokio::test]
async fn file_tools_refuse_the_data_root_when_they_run() {
    let site = Site::new();
    std::fs::write(site.0.join("data/token"), "local-token-for-test").expect("写得了");
    let token = site.real("data").join("token");
    let judged = vec![site.real("work").join("token")];
    let read = site
        .done_checked(
            "read",
            json!({"file_path": token.to_string_lossy()}),
            None,
            Some(judged),
        )
        .await;
    assert!(read.error, "read 执行时读出了数据根里的文件：{read:?}");
    assert!(!text(&read).contains("local-token-for-test"), "{read:?}");
    assert!(
        text(&read).contains("changed after the permission check"),
        "{read:?}"
    );
    let planted = site.real("data").join("planted.txt");
    let judged = vec![site.real("work").join("planted.txt")];
    let write = site
        .done_checked(
            "write",
            json!({"file_path": planted.to_string_lossy(), "content": "x"}),
            None,
            Some(judged),
        )
        .await;
    assert!(write.error, "write 执行时写进了数据根：{write:?}");
    assert!(!planted.exists(), "数据根里多了文件");
}

/// 判的时候 `work/link` 指着工作区里的 `safe/`，判完被换成指进数据根：读、写都不跟过去。
#[cfg(unix)]
#[tokio::test]
async fn a_link_swapped_after_the_check_does_not_lead_into_the_data_root() {
    use std::os::unix::fs::symlink;

    let site = Site::new();
    site.file("work/safe/token", b"harmless");
    std::fs::write(site.0.join("data/token"), "local-token-for-test").expect("写得了");
    let link = site.0.join("work/link");
    symlink(site.0.join("work/safe"), &link).expect("链得上");
    let judged = vec![
        site.real("work/safe/token"),
        site.real("work/safe").join("planted.txt"),
    ];
    std::fs::remove_file(&link).expect("删得掉");
    symlink(site.0.join("data"), &link).expect("链得上");
    let read = site
        .done_checked(
            "read",
            json!({"file_path": "link/token"}),
            None,
            Some(judged.clone()),
        )
        .await;
    assert!(read.error, "{read:?}");
    assert!(!text(&read).contains("local-token-for-test"), "{read:?}");
    let write = site
        .done_checked(
            "write",
            json!({"file_path": "link/planted.txt", "content": "x"}),
            None,
            Some(judged),
        )
        .await;
    assert!(write.error, "{write:?}");
    assert!(
        !site.0.join("data/planted.txt").exists(),
        "数据根里多了文件"
    );
}
