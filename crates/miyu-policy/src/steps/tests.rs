//! 步数上限（施工 AE-1）：四种会话给的数、预设写了的数、以前造的快照读出来的数，和造出的内核策略。

use super::*;
use crate::compose::compose;
use crate::test_support::sources;

/// 照 `attended` 拼的快照造出的内核策略里的两个数。
fn kernel(snapshot: &Snapshot) -> (Option<u32>, Option<u32>) {
    let policy = snapshot.policy().unwrap();
    (policy.step_limit, policy.auto_step_limit)
}

#[test]
fn each_kind_of_session_gets_its_numbers() {
    let cases = [
        (Watched::Attended, (None, Some(500))),
        (Watched::Unattended, (Some(500), Some(500))),
        (Watched::Subagent, (Some(800), Some(800))),
        (Watched::Fork, (Some(500), Some(500))),
    ];
    for (watched, (every, auto)) in cases {
        assert_eq!(
            StepLimits::of(watched, None),
            StepLimits { every, auto },
            "{watched:?}"
        );
    }
}

#[test]
fn a_preset_sets_the_unattended_number_and_not_the_subagents() {
    let of = |watched| StepLimits::of(watched, Some(40));
    assert_eq!(
        of(Watched::Attended),
        StepLimits {
            every: None,
            auto: Some(40)
        }
    );
    assert_eq!(
        of(Watched::Unattended),
        StepLimits {
            every: Some(40),
            auto: Some(40)
        }
    );
    assert_eq!(
        of(Watched::Subagent),
        StepLimits::of(Watched::Subagent, None)
    );
    assert_eq!(of(Watched::Fork), StepLimits::of(Watched::Fork, None));
}

/// 审计的红测试（2026-10-30，缺陷 AE-1）：没人在场造出的快照有步数上限；有人在场的人开的回合照旧不限。
#[test]
fn an_unattended_snapshot_has_a_step_limit() {
    let unattended = compose(Some("engineer"), sources(), false);
    assert_eq!(
        unattended.steps(),
        StepLimits::of(Watched::Unattended, None)
    );
    assert_eq!(kernel(&unattended), (Some(500), Some(500)));
    let attended = compose(Some("engineer"), sources(), true);
    assert_eq!(kernel(&attended), (None, Some(500)));
}

#[test]
fn the_numbers_go_into_the_bytes_and_read_back() {
    let snapshot = compose(Some("engineer"), sources(), false)
        .with_steps(StepLimits::of(Watched::Unattended, Some(40)));
    let text = String::from_utf8(snapshot.to_bytes()).unwrap();
    assert!(
        text.contains(r#","step_limit":40,"auto_step_limit":40,"attended":false,"#),
        "{text}"
    );
    let back = Snapshot::from_bytes(&snapshot.to_bytes()).unwrap();
    assert_eq!(back, snapshot);
    assert_eq!(kernel(&back), (Some(40), Some(40)));
}

/// 以前造的快照（日志里 `session.created` 记的那份）没有 `auto_step_limit`、`step_limit` 是 `null`：不读成不限，照 `attended`
/// 读成出厂的；字节原样，哈希不变。
#[test]
fn an_older_snapshot_reads_as_the_factory_numbers_for_its_kind() {
    for (attended, want) in [(true, (None, Some(500))), (false, (Some(500), Some(500)))] {
        let new = compose(Some("engineer"), sources(), attended);
        let text = String::from_utf8(new.to_bytes()).unwrap();
        let written = serde_json::to_string(&new.step_limit).unwrap();
        let old_text = text.replace(
            &format!(r#""step_limit":{written},"auto_step_limit":500,"#),
            r#""step_limit":null,"#,
        );
        assert_ne!(old_text, text, "照以前的写法改出来了");
        let old = Snapshot::from_bytes(old_text.as_bytes()).unwrap();
        assert_eq!(
            old.steps(),
            StepLimits {
                every: None,
                auto: None
            }
        );
        assert_eq!(old.to_bytes(), old_text.as_bytes(), "字节原样，哈希不变");
        assert_eq!(kernel(&old), want, "attended {attended}");
    }
}
