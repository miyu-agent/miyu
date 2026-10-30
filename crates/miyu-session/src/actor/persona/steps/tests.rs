//! 改了预设的 `[limits]`，下一个回合换上的快照照新的数（施工 AE-1）：有人看着的只改自己开的回合那个数，没人看着的两个都改；
//! 子代理的不跟；发给模型的 system、工具面一字不变。

use super::super::test_support::{look_now, setup, write_preset};
use super::super::{Refresh, Seen};
use super::*;

/// 换上的快照、它的两个数、照它造出的内核策略的两个数。
type Looked = (Snapshot, StepLimits, (Option<u32>, Option<u32>));

/// 写 `[limits]` 以后看一眼：换了的交回 [`Looked`]；没换的是没有。
fn looked(refresh: &Refresh) -> Option<Looked> {
    match look_now(refresh) {
        Seen::Swapped(snapshot, policy, ..) => {
            let steps = snapshot.steps();
            Some((
                *snapshot,
                steps,
                (policy.step_limit, policy.auto_step_limit),
            ))
        }
        _ => None,
    }
}

#[test]
fn a_changed_limit_is_swapped_in_next_turn() {
    let (_scratch, root, mut refresh) = setup("steps-attended", "");
    assert_eq!(
        refresh.snapshot.steps(),
        StepLimits::of(Watched::Attended, None)
    );
    write_preset(&root, "[limits]\nunattended_steps = 40\n");
    let (snapshot, steps, kernel) = looked(&refresh).expect("改了要换");
    assert_eq!(steps, StepLimits::of(Watched::Attended, Some(40)));
    assert_eq!(kernel, (None, Some(40)), "人开的回合照旧不限");
    assert_eq!(snapshot.system, refresh.snapshot.system, "system 一字不变");
    assert_eq!(snapshot.tools, refresh.snapshot.tools, "工具面一字不变");
    // 没人看着的会话两个都照它。
    refresh.snapshot.attended = false;
    let (_, steps, kernel) = looked(&refresh).expect("改了要换");
    assert_eq!(steps, StepLimits::of(Watched::Unattended, Some(40)));
    assert_eq!(kernel, (Some(40), Some(40)));
}

#[test]
fn a_subagent_keeps_its_own_numbers() {
    let (_scratch, root, mut refresh) = setup("steps-child", "");
    refresh.child = true;
    refresh.snapshot = refresh
        .snapshot
        .clone()
        .with_steps(StepLimits::of(Watched::Subagent, None));
    write_preset(&root, "[limits]\nunattended_steps = 40\n");
    let (_, steps, _) = looked(&refresh).expect("预设改了照样换");
    assert_eq!(
        steps,
        StepLimits::of(Watched::Subagent, None),
        "子代理不看预设"
    );
}
