//! 步数上限（施工 AE-1，`docs/blueprint/policy.md`「对外的样子」）：造会话时照会话的种类、预设给两个数，记进快照；没人看着的
//! 会话照预设写的数停。真会话、真日志，数据根在临时目录。

use miyu_kernel::event::{Body, EndReason, Event};
use miyu_kernel::id::SessionId;
use miyu_policy::Snapshot;
use miyu_policy::features::Features;
use miyu_policy::preset::{Chosen, PresetFile};
use miyu_policy::steps::{StepLimits, Watched};
use miyu_session::Lineage;
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;
use miyu_tool::Catalog;

use crate::support::*;

/// 真的基础系统：一直调 `read` 用。
fn basesystem(home: &Home) -> Catalog {
    Catalog::new(miyu_basesystem::tools(home.resources.path()).expect("读得出")).expect("合写法")
}

/// 会话 `session` 造会话时的策略快照。
fn snapshot(home: &Home, session: &SessionId) -> Snapshot {
    let log = home.log(session);
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第 1 条应该是造会话：{:?}", log[0]);
    };
    let bytes = Blobs::new(home.root.blobs())
        .get(&created.policy)
        .expect("快照在 blob 里");
    Snapshot::from_bytes(&bytes).expect("读得懂")
}

/// `[limits] unattended_steps` 写了 `steps` 的预设。
fn limited(steps: u32) -> Chosen {
    let file = PresetFile {
        unattended_steps: Some(steps),
        ..PresetFile::default()
    };
    Chosen::new("p".to_string(), file, &Features::default())
}

/// 一直调读的替身：`n` 次。
fn reading(n: usize) -> Script {
    Script::new((0..n).map(|_| Play::calls(&[("read", r#"{"path":"nope.txt"}"#)])))
}

/// 日志里第一个结束的回合：怎么结束的、这一轮请求了几次。
fn first_turn(log: &[Event]) -> (Option<EndReason>, usize) {
    let ended = log.iter().find_map(|event| match &event.body {
        Body::TurnEnded(ended) => Some(ended.reason.clone()),
        _ => None,
    });
    let called = log
        .iter()
        .filter(|event| matches!(event.body, Body::ModelCalled(_)))
        .count();
    (ended, called)
}

#[tokio::test]
async fn an_attended_session_leaves_turns_people_open_unlimited() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let handle = home
        .create_full(
            &script,
            &basesystem(&home),
            Opening::default(),
            Lines {
                preset: Some(limited(40)),
                ..Lines::default()
            },
        )
        .await;
    assert_eq!(
        snapshot(&home, handle.id()).steps(),
        StepLimits::of(Watched::Attended, Some(40)),
        "人开的回合不限，自己开的照预设"
    );
}

#[tokio::test]
async fn an_unattended_session_stops_at_the_number_its_preset_writes() {
    let home = Home::new();
    let script = reading(5);
    let opening = Opening {
        attended: false,
        ..Opening::default()
    };
    let handle = home
        .create_full(
            &script,
            &basesystem(&home),
            opening,
            Lines {
                preset: Some(limited(2)),
                ..Lines::default()
            },
        )
        .await;
    assert_eq!(
        snapshot(&home, handle.id()).steps(),
        StepLimits::of(Watched::Unattended, Some(2))
    );
    ask(&handle, "cmd-1", say("一直读"))
        .await
        .expect("会话在跑");
    let log = until_logged(&home, handle.id(), |log| {
        log.iter()
            .any(|event| matches!(event.body, Body::TurnEnded(_)))
    })
    .await;
    assert_eq!(first_turn(&log), (Some(EndReason::StepLimit), 2));
}

#[tokio::test]
async fn an_unattended_session_without_a_preset_number_gets_the_factory_one() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let opening = Opening {
        attended: false,
        ..Opening::default()
    };
    let handle = home
        .create_full(&script, &basesystem(&home), opening, Lines::default())
        .await;
    assert_eq!(
        snapshot(&home, handle.id()).steps(),
        StepLimits::of(Watched::Unattended, None)
    );
}

#[tokio::test]
async fn a_subagent_gets_its_own_number_whatever_the_preset() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let handle = home
        .create_full(
            &script,
            &basesystem(&home),
            Opening::default(),
            Lines {
                lineage: Some(Lineage {
                    parent: SessionId::parse("0192f3a0-1111-7abc-8def-55669f03b21c").unwrap(),
                    depth: 1,
                    workspace_from: None,
                }),
                preset: Some(limited(40)),
                ..Lines::default()
            },
        )
        .await;
    assert_eq!(
        snapshot(&home, handle.id()).steps(),
        StepLimits::of(Watched::Subagent, None)
    );
}
