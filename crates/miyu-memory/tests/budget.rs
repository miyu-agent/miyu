//! 记忆这个软件包的工具面预算（`docs/designs/10-自带软件.md` 第九节，施工 R-3 中）：`resources/software/memory/tools/` 下的
//! 三份说明加起来不超过 [`BUDGET`] 字节。
//!
//! 照基础系统的办法：2026-10-07 照 magpie 网关的 `cline-pass/deepseek-v4.1-flash` 量，十八件（基础系统十五件加这三件）一起时
//! 的边际份量 `remember` 149、`forget` 66、`memory_search` 70，合计 285 个 token，说明合计 1006 字节；预算是实测加一成，
//! 314 个 token，合 1100 字节。施工 R-15 上 `remember` 多一格 `keywords`：2026-10-19 照同一个网关的
//! `group/auto-deepseek-v4-1-flash` 量，`remember` 149 → 184，三件合计 320 个 token、1143 字节；预算照实测加一成改成 352 个
//! token，合 1260 字节。加工具、改说明超了，重新量过再改这里和设计。

use std::path::Path;

/// 预算：字节，回车 `\r` 不算（Windows 上检出的可能多出回车）。
const BUDGET: usize = 1260;

#[test]
fn the_memory_tool_face_stays_within_its_budget() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/software/memory/tools");
    let mut total = 0;
    let mut files = 0;
    for entry in std::fs::read_dir(&dir).expect("读得了工具说明的目录") {
        let path = entry.expect("读得了目录项").path();
        let bytes = std::fs::read(&path).expect("读得了说明");
        total += sent(&bytes);
        files += 1;
    }
    assert_eq!(files, 3, "三件工具各一份说明");
    assert!(
        total <= BUDGET,
        "三份说明合计 {total} 字节，超过预算 {BUDGET}"
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
