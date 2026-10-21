//! 知识库这个软件包的工具面预算（`docs/designs/10-自带软件.md` 第九节，施工 K-1 下、K-3）：`resources/software/knowledge/tools/`
//! 下的五份说明加起来不超过 [`BUDGET`] 字节。
//!
//! 照记忆那个包的办法：2026-10-12 照 magpie 网关的 `clinepass/cline-pass/deepseek-v4.1-flash` 量，二十五件（基础系统十六件、
//! 记忆三件、长期目标一件加这五件）一起时的边际份量 `kb_search` 75、`kb_read` 87、`kb_save` 139、`kb_edit` 93、`kb_delete`
//! 63，合计 457 个 token，说明合计 1506 字节；预算是实测加一成，503 个 token，合 1660 字节（K-1 下两件时是 600）。加工具、
//! 改说明超了，重新量过再改这里和设计。

use std::path::Path;

/// 预算：字节，回车 `\r` 不算（Windows 上检出的可能多出回车）。
const BUDGET: usize = 1660;

#[test]
fn the_knowledge_tool_face_stays_within_its_budget() {
    let dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/software/knowledge/tools");
    let mut total = 0;
    let mut files = 0;
    for entry in std::fs::read_dir(&dir).expect("读得了工具说明的目录") {
        let path = entry.expect("读得了目录项").path();
        let bytes = std::fs::read(&path).expect("读得了说明");
        total += sent(&bytes);
        files += 1;
    }
    assert_eq!(files, 5, "五件工具各一份说明");
    assert!(
        total <= BUDGET,
        "五份说明合计 {total} 字节，超过预算 {BUDGET}"
    );
}

/// 进 tools 数组的字节数：回车不算，`"result"` 那一行不算（施工 M-4：结果的样子不直接发给模型，换进 `shell` 说明那一段，
/// 在那边登记）。
fn sent(bytes: &[u8]) -> usize {
    String::from_utf8_lossy(bytes)
        .split_inclusive('\n')
        .filter(|line| !line.trim_start().starts_with("\"result\""))
        .map(|line| line.bytes().filter(|byte| *byte != b'\r').count())
        .sum()
}
