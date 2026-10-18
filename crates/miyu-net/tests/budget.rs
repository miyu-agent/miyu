//! 联网这个软件包的工具面预算（设计 36，施工 N-1）：`resources/software/net/tools/` 下的说明加起来不超过 [`BUDGET`] 字节。
//!
//! 照知识库那个包的办法：2026-10-18 照 magpie 网关的 `clinepass/cline-pass/deepseek-v4.1-flash` 量，`web_fetch` 和 `trash` 一起时的
//! 边际份量 84 个 token，说明 257 字节；预算是实测加一成，283 字节。加工具、改说明超了，重新量过再改这里和设计。

use std::path::Path;

/// 预算：字节，回车 `\r` 不算（Windows 上检出的可能多出回车）。
const BUDGET: usize = 283;

#[test]
fn the_net_tool_face_stays_within_its_budget() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/software/net/tools");
    let mut total = 0;
    let mut files = 0;
    for entry in std::fs::read_dir(&dir).expect("读得了工具说明的目录") {
        let path = entry.expect("读得了目录项").path();
        let bytes = std::fs::read(&path).expect("读得了说明");
        total += bytes.iter().filter(|byte| **byte != b'\r').count();
        files += 1;
    }
    assert_eq!(files, 1, "一件工具一份说明");
    assert!(total <= BUDGET, "说明合计 {total} 字节，超过预算 {BUDGET}");
}
