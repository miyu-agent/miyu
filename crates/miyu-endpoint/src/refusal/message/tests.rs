//! 每一个原因码都有给人看的整句（施工 CLI-SAID-1）：协议参考 `docs/zh/reference/errors.md` 里列着的、内核会拒的（`Reason::code`），
//! 中文、英文都不是兜底的那一句。新加原因码忘了写话的，头和命令行只印得出「已拒绝。」。

use super::{of, said};

/// 协议参考里列着的原因码：每张表第一格里反引号括着的（一格可以列几个），表头的 `reason` 不算。
fn documented() -> Vec<&'static str> {
    let page = include_str!("../../../../../docs/zh/reference/errors.md");
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

/// 施工 WD-7：`command_not_allowed` 有三种情形共用：身份不够的说「仅主人和群管理员可用」（`data.why` 没有，或者就是这一句）；
/// 命令停用了、预设没开这个技能或模板的 `data.why` 另说了原因，那时说一句不带原因的，不和 `data.why` 说的打架。`local_only` 拒的有扩展、设备
/// 令牌连上的连接、越权改配置的软件包：说的是「谁可以」，不是「在哪里」。
#[test]
fn sentences_that_cover_every_case_they_are_used_for() {
    assert_eq!(
        said("command_not_allowed", None).0,
        "仅主人和群管理员可用。"
    );
    for same in ["仅主人和群管理员可用。", "Owner and group admins only."] {
        let mut data = serde_json::Map::new();
        data.insert("why".to_string(), serde_json::json!(same));
        assert_eq!(
            said("command_not_allowed", Some(&data)).0,
            "仅主人和群管理员可用。",
            "身份不够的 data.why 就是这一句"
        );
    }
    let mut data = serde_json::Map::new();
    data.insert("why".to_string(), serde_json::json!("/stop 已停用。"));
    let (zh, en) = said("command_not_allowed", Some(&data));
    assert_eq!(
        (zh.as_ref(), en.as_ref()),
        ("不能使用这条命令。", "This command can't be used.")
    );
    assert_eq!(
        of("local_only"),
        (
            "只有本机上的用户可以执行这个操作。",
            "Only a user on this machine can do this."
        )
    );
}
