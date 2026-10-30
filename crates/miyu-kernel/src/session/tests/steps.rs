//! 步数上限照开轮的是谁（施工 AE-1，`docs/blueprint/kernel/session.md`「步数上限」）：有人看着的会话里人开的回合不限；定时任务、
//! 长期目标（模块）开的回合照「自己开的回合」那个数停；两个都有的取小的；换策略时这一轮照新的那一份重算。

use std::collections::VecDeque;

use super::executor::*;
use super::*;
use crate::event::EndReason;
use crate::id::{ContentHash, ModuleId};
use crate::origin::{Module, Schedule};

/// 替身一直调读的：最多请求这么多次就不再答，当成停不下来。
const MOST: u32 = 12;

/// 有人看着、人开的回合不限、自己开的回合最多 `auto` 次的策略。
fn attended(auto: u32) -> Policy {
    let mut policy = policy();
    policy.step_limit = None;
    policy.auto_step_limit = Some(auto);
    policy
}

/// 编号是 `n` 的命令：`by` 说一句话。
fn said(n: u64, by: By) -> Input {
    Input::Command(Received {
        id: id(n),
        by,
        at: at(n % 60),
        command: Command::Send {
            blocks: vec![Block::Text(Text {
                text: "go on".to_string(),
            })],
            urgent: false,
            venue: None,
        },
    })
}

/// 到点的定时任务。
fn schedule() -> By {
    By::Schedule(Schedule {
        schedule: "t1".to_string(),
        looping: None,
    })
}

/// 长期目标的续轮：模块 `goal` 交进来的（`miyu-session` 的 `actor/goal.rs`）。
fn goal() -> By {
    By::Module(Module {
        id: ModuleId::parse("goal").unwrap(),
    })
}

/// 送进 `first`，执行器替身照常走：落了盘就报落盘，挂接点交回空的（带着 `policy` 的换上放着的策略），每次请求都回一件
/// `read`，跑完接着来。交回请求了几次、回合以什么结束；请求到 [`MOST`] 次还没结束的是没有。
fn drive(
    session: &mut Session,
    first: Input,
    policy: Option<ContentHash>,
) -> (u32, Option<EndReason>) {
    let mut inputs = VecDeque::from([first]);
    let mut requests = 0;
    while let Some(input) = inputs.pop_front() {
        let mut actions: VecDeque<Action> = allowing(session, input).into();
        while let Some(action) = actions.pop_front() {
            match action {
                Action::Append(events) => {
                    if let Some(ended) = events.iter().find_map(|event| match &event.body {
                        Body::TurnEnded(ended) => Some(ended.reason.clone()),
                        _ => None,
                    }) {
                        return (requests, Some(ended));
                    }
                    if let Some(last) = events.last() {
                        inputs.push_back(stored(last.seq.get()));
                    }
                }
                Action::RunTurnStartHooks { turn, .. } => {
                    let mut hooked = hooks_done(turn, Vec::new());
                    if let Input::TurnStartHooksDone {
                        policy: swapped, ..
                    } = &mut hooked
                    {
                        swapped.clone_from(&policy);
                    }
                    inputs.push_back(hooked);
                }
                Action::CallModel { seen, .. } => {
                    requests += 1;
                    if requests > MOST {
                        return (requests, None);
                    }
                    actions.extend(call_tools(session, seen.get(), &[("read", "{}")]));
                }
                Action::RunTool { call_id, .. } => inputs.push_back(done(call_id, "a")),
                _ => {}
            }
        }
    }
    (requests, None)
}

#[test]
fn a_scheduled_turn_stops_at_its_limit_and_a_persons_does_not() {
    let mut session = session_with(attended(3));
    assert_eq!(
        drive(&mut session, said(1, schedule()), None),
        (3, Some(EndReason::StepLimit)),
        "定时任务开的回合到第 3 次请求停"
    );
    let mut session = session_with(attended(3));
    assert_eq!(
        drive(&mut session, said(1, person()), None),
        (MOST + 1, None),
        "人开的回合不限"
    );
}

#[test]
fn a_goal_round_stops_at_its_limit() {
    let mut session = session_with(attended(2));
    assert_eq!(
        drive(&mut session, said(1, goal()), None),
        (2, Some(EndReason::StepLimit))
    );
}

#[test]
fn the_smaller_of_the_two_limits_wins_for_a_turn_nobody_opened() {
    for (every, auto, want) in [(4, 2, 2), (2, 4, 2)] {
        let limited = || {
            let mut policy = policy();
            policy.step_limit = Some(every);
            policy.auto_step_limit = Some(auto);
            policy
        };
        let mut session = session_with(limited());
        assert_eq!(
            drive(&mut session, said(1, schedule()), None),
            (want, Some(EndReason::StepLimit)),
            "每一轮 {every}、自己开的 {auto}"
        );
        // 人开的只照每一轮的那个数。
        let mut session = session_with(limited());
        assert_eq!(
            drive(&mut session, said(1, person()), None),
            (every, Some(EndReason::StepLimit))
        );
    }
}

#[test]
fn a_policy_swapped_at_the_start_of_a_turn_sets_its_limit() {
    let mut session = session_with(attended(5));
    session.stage_policy(attended(1));
    let hash = ContentHash::of(b"new snapshot");
    assert_eq!(
        drive(&mut session, said(1, schedule()), Some(hash)),
        (1, Some(EndReason::StepLimit)),
        "换上的策略说自己开的回合最多 1 次：这一轮就照它"
    );
}
