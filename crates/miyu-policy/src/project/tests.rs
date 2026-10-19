//! 项目的附加提示词（施工 H-3）：空的没有，有的照模板放进去、去掉首尾空白；带上的接在 system 后面，没有的一字不变。

use super::*;
use crate::test_support::engineer;

const TEMPLATE: &str = "<project_instructions>\n{prompt}\n</project_instructions>\n";

#[test]
fn an_empty_prompt_has_no_block_and_leaves_the_system_alone() {
    assert_eq!(block(TEMPLATE, " \n "), None);
    assert_eq!(
        block(TEMPLATE, "\n Reply in Chinese. \n"),
        Some("<project_instructions>\nReply in Chinese.\n</project_instructions>\n".to_string())
    );
    let plain = engineer();
    let same = plain.clone().with_project(None);
    assert_eq!(same.system, plain.system);
    assert_eq!(same.to_bytes(), plain.to_bytes(), "没有的字节一样");
    let with = plain
        .clone()
        .with_project(block(TEMPLATE, "Reply in Chinese."));
    assert!(
        with.system
            .ends_with("<project_instructions>\nReply in Chinese.\n</project_instructions>"),
        "{}",
        with.system
    );
    assert!(with.system.starts_with(&plain.system));
    assert!(with.project.is_some());
}
