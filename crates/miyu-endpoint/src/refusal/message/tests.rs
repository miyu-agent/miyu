//! 每一个原因码都有给人看的整句（施工 CLI-SAID-1）：协议参考 `docs/reference/errors.md` 里列着的、内核会拒的（`Reason::code`），
//! 中文、英文都不是兜底的那一句。新加原因码忘了写话的，头和命令行只印得出「已拒绝。」（项目主人 2026-11-17 实测里
//! `miyu schedule rm` 删人生那一行就是这样）。

use super::of;

/// 协议参考里列着的原因码：每张表第一格里反引号括着的（一格可以列几个），表头的 `reason` 不算。
fn documented() -> Vec<&'static str> {
    let page = include_str!("../../../../../docs/reference/errors.md");
    let mut codes = Vec::new();
    for line in page.lines().filter(|line| line.starts_with("| `")) {
        let first = line.split('|').nth(1).unwrap_or_default();
        for (at, part) in first.split('`').enumerate() {
            let code = part.trim();
            let named = at % 2 == 1 && !code.is_empty();
            if named
                && code != "reason"
                && code.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
            {
                codes.push(code);
            }
        }
    }
    codes
}

/// 内核会拒的原因码：`Reason::code` 里每一臂箭头后面的那个字。
fn kernel() -> Vec<&'static str> {
    let source = include_str!("../../../../miyu-kernel/src/session/action.rs");
    source
        .lines()
        .filter_map(|line| line.trim().strip_prefix("Reason::"))
        .filter_map(|arm| arm.split_once("=> \""))
        .filter_map(|(_, rest)| rest.split('"').next())
        .collect()
}

#[test]
fn every_reason_has_a_sentence_in_both_languages() {
    let (documented, kernel) = (documented(), kernel());
    assert!(
        documented.len() > 150,
        "读得出协议参考里的表：{}",
        documented.len()
    );
    assert!(kernel.len() > 30, "读得出内核的原因码：{}", kernel.len());
    let silent: Vec<&str> = documented
        .into_iter()
        .chain(kernel)
        .filter(|code| of(code) == ("已拒绝。", "Refused."))
        .collect();
    assert!(silent.is_empty(), "这几个原因码没有给人看的话：{silent:?}");
}
