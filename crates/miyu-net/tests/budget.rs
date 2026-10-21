//! 联网这个软件包的工具面预算（设计 36，施工 N-1、N-2 上）：`resources/software/net/tools/` 下的说明加起来不超过 [`BUDGET`] 字节。
//!
//! 照知识库那个包的办法：2026-10-18 照 magpie 网关的 `clinepass/cline-pass/deepseek-v4.1-flash` 量，和 `trash` 一起时的边际份量
//! `web_fetch` 84、`web_search` 107 个 token，说明 257、346 字节，合计 603；预算是实测加一成，663 字节（N-1 一件时是 283）。
//! 加工具、改说明超了，重新量过再改这里和设计。

use std::path::Path;

/// 预算：字节，回车 `\r` 不算（Windows 上检出的可能多出回车）。
const BUDGET: usize = 663;

#[test]
fn the_net_tool_face_stays_within_its_budget() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/software/net/tools");
    let mut total = 0;
    let mut files = 0;
    for entry in std::fs::read_dir(&dir).expect("读得了工具说明的目录") {
        let path = entry.expect("读得了目录项").path();
        let bytes = std::fs::read(&path).expect("读得了说明");
        total += sent(&bytes);
        files += 1;
    }
    assert_eq!(files, 2, "两件工具各一份说明");
    assert!(total <= BUDGET, "说明合计 {total} 字节，超过预算 {BUDGET}");
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
