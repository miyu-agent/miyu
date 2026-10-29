//! 到点交进来的话（施工 J-1 上）：定时任务、固定循环包定时那一对，她自己定节奏的包循环那一对，标签里是任务编号；附件接在
//! 后面；没有字的（以前的快照、这一版还没写字）原样交回；进请求时照 `by` 包。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::origin::{By, Schedule, ScheduleLoop};

use super::*;
use crate::test_support::texts;

fn words(text: &str) -> Block {
    Block::Text(Text {
        text: text.to_string(),
    })
}

fn task(looping: Option<ScheduleLoop>) -> Schedule {
    Schedule {
        schedule: "t3".to_string(),
        looping,
    }
}

#[test]
fn each_kind_gets_its_own_pair_with_the_task_id() {
    let texts = texts().schedule.unwrap();
    assert_eq!(
        message(&task(None), vec![words("看看 CI")], Some(&texts)),
        [words("<task t3>\n看看 CI\n</task>\n")]
    );
    assert_eq!(
        message(
            &task(Some(ScheduleLoop::Fixed)),
            vec![words("看看 CI")],
            Some(&texts)
        ),
        [words("<task t3>\n看看 CI\n</task>\n")],
        "固定循环同定时任务"
    );
    assert_eq!(
        message(
            &task(Some(ScheduleLoop::Dynamic)),
            vec![words("看看 CI\n")],
            Some(&texts)
        ),
        [words("<wakeup t3>\n看看 CI\nagain?\n</wakeup>\n")],
        "末尾有换行的不再补"
    );
}

#[test]
fn without_texts_it_is_left_as_said() {
    assert_eq!(
        message(&task(None), vec![words("看看 CI")], None),
        [words("看看 CI")]
    );
}

#[test]
fn the_by_is_what_picks_it() {
    let by = By::Schedule(task(Some(ScheduleLoop::Dynamic)));
    let json = serde_json::to_value(&by).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"kind": "schedule", "schedule": "t3", "loop": "dynamic"})
    );
    let back: By = serde_json::from_value(json).unwrap();
    assert_eq!(back, by, "读得回原样");
    let plain = serde_json::to_value(By::Schedule(task(None))).unwrap();
    assert!(plain.get("loop").is_none(), "定时任务不写 loop");
}
