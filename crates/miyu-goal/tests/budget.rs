//! 长期目标这个软件包的工具面预算（`docs/designs/10-自带软件.md` 第九节，施工 X-8 上）：`resources/software/goal/tools/` 下的
//! 一份说明不超过 [`BUDGET`] 字节。
//!
//! 照记忆的办法：量法、实测的 token 数记在设计里；预算是实测加一成。改说明超了，重新量过再改这里和设计。

use std::path::Path;

/// 预算：字节，回车 `\r` 不算（Windows 上检出的可能多出回车）。
const BUDGET: usize = 690;

#[test]
fn the_goal_tool_face_stays_within_its_budget() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/software/goal/tools");
    let mut total = 0;
    let mut files = 0;
    for entry in std::fs::read_dir(&dir).expect("读得了工具说明的目录") {
        let path = entry.expect("读得了目录项").path();
        let bytes = std::fs::read(&path).expect("读得了说明");
        total += bytes.iter().filter(|byte| **byte != b'\r').count();
        files += 1;
    }
    assert_eq!(files, 1, "一件工具一份说明");
    assert!(total <= BUDGET, "说明 {total} 字节，超过预算 {BUDGET}");
}
