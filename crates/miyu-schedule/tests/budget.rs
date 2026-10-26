//! 定时任务、循环两个软件包的工具面预算（`docs/designs/10-自带软件.md` 第九节，施工 J-2）：`resources/software/<包>/tools/` 下的
//! 说明加起来不超过各自的预算字节。
//!
//! 照记忆、长期目标的办法：量法、实测的 token 数记在登记簿里；预算是实测加一成。改说明超了，重新量过再改这里和登记簿。

use std::path::Path;

/// 包、几件、预算字节（回车 `\r` 不算：Windows 上检出的可能多出回车）。
const BUDGETS: [(&str, usize, usize); 2] = [("schedule", 3, 1_065), ("loop", 1, 588)];

#[test]
fn the_schedule_and_loop_tool_faces_stay_within_their_budgets() {
    for (package, count, budget) in BUDGETS {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../resources/software")
            .join(package)
            .join("tools");
        let mut total = 0;
        let mut files = 0;
        for entry in std::fs::read_dir(&dir).expect("读得了工具说明的目录") {
            let path = entry.expect("读得了目录项").path();
            let bytes = std::fs::read(&path).expect("读得了说明");
            total += bytes.iter().filter(|byte| **byte != b'\r').count();
            files += 1;
        }
        assert_eq!(files, count, "{package}：一件工具一份说明");
        assert!(
            total <= budget,
            "{package}：说明 {total} 字节，超过预算 {budget}"
        );
    }
}
