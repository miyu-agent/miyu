//! 守卫的结论合上策略的（施工 HK-1，设计 65 第三节第 4 条）：最严者胜，只能收紧。

use serde_json::json;

use miyu_kernel::event::Said;
use miyu_kernel::id::ModuleId;
use miyu_kernel::raw::RawJson;
use miyu_kernel::session::Verdict;
use miyu_kernel::tool::{Access, Worded};
use miyu_tool::Guarded;

use super::merged;

fn module(id: &str) -> ModuleId {
    ModuleId::parse(id).unwrap()
}

/// 策略要问人：带着放行规则。
fn policy_asks() -> Verdict {
    Verdict::Ask {
        module: module("permissions"),
        access: Access::Write,
        rule: RawJson::of(&json!({"tool": "write", "write": ["/w"]})),
        detail: None,
    }
}

fn guard_denies() -> Guarded {
    Guarded::Deny {
        module: module("acme-policy"),
        worded: Worded {
            text: "Blocked by acme-policy: no\n".to_string(),
            said: Some(Said::new("core/guard/denied-with-reason")),
        },
    }
}

fn guard_asks() -> Guarded {
    Guarded::Ask {
        module: module("acme-policy"),
        detail: RawJson::of(&json!({"guard": "acme-policy", "tool": "write"})),
    }
}

#[test]
fn a_denial_wins_over_anything_the_policy_said() {
    for policy in [Verdict::Allow, policy_asks()] {
        let Verdict::Deny {
            module,
            text,
            human,
        } = merged(policy, guard_denies(), Access::Write)
        else {
            panic!("守卫拒绝就拒绝");
        };
        assert_eq!(module.as_str(), "acme-policy", "by 是那个包");
        assert_eq!(text, "Blocked by acme-policy: no\n");
        assert_eq!(human, Some(Said::new("core/guard/denied-with-reason")));
    }
}

#[test]
fn a_guard_that_asks_turns_an_allow_into_asking_without_a_rule() {
    let Verdict::Ask {
        module,
        access,
        rule,
        detail,
    } = merged(Verdict::Allow, guard_asks(), Access::Execute)
    else {
        panic!("策略放行、守卫要问人的问人");
    };
    assert_eq!(module.as_str(), "acme-policy");
    assert_eq!(access, Access::Execute, "要的是这件工具的访问类别");
    assert_eq!(rule, None, "不提规则：只能允许这一次或者拒绝");
    assert!(detail.is_some());
}

#[test]
fn a_guard_never_loosens_the_policy() {
    assert_eq!(
        merged(policy_asks(), Guarded::Allow, Access::Write),
        policy_asks(),
        "策略问人、守卫放行的照旧问人"
    );
    assert_eq!(
        merged(policy_asks(), guard_asks(), Access::Write),
        policy_asks(),
        "两边都问人的照策略的问：它带着放行规则"
    );
    assert_eq!(
        merged(Verdict::Allow, Guarded::Allow, Access::Read),
        Verdict::Allow
    );
}
