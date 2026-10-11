//! 知识库这个软件包的工具面预算（`docs/designs/10-自带软件.md` 第九节，施工 K-1 下）：`resources/software/knowledge/tools/`
//! 下的两份说明加起来不超过 [`BUDGET`] 字节。
//!
//! 照记忆那个包的办法：2026-10-12 照 magpie 网关的 `clinepass/cline-pass/deepseek-v4.1-flash` 量，二十一件（基础系统十六件、
//! 记忆三件加这两件）一起时的边际份量 `kb_search` 75、`kb_read` 87，合计 162 个 token，说明合计 541 字节；预算是实测加一成，
//! 178 个 token，合 600 字节。加工具、改说明超了，重新量过再改这里和设计。

use std::path::Path;

/// 预算：字节，回车 `\r` 不算（Windows 上检出的可能多出回车）。
const BUDGET: usize = 600;

#[test]
fn the_knowledge_tool_face_stays_within_its_budget() {
    let dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/software/knowledge/tools");
    let mut total = 0;
    let mut files = 0;
    for entry in std::fs::read_dir(&dir).expect("读得了工具说明的目录") {
        let path = entry.expect("读得了目录项").path();
        let bytes = std::fs::read(&path).expect("读得了说明");
        total += bytes.iter().filter(|byte| **byte != b'\r').count();
        files += 1;
    }
    assert_eq!(files, 2, "两件工具各一份说明");
    assert!(
        total <= BUDGET,
        "两份说明合计 {total} 字节，超过预算 {BUDGET}"
    );
}
