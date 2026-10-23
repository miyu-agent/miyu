//! 等人确认的那几段不算时限（施工 M-7 上）：在等的时候到了时限也不杀、不挪；等完了接着算剩下的；在等时结束照常交回。
//! 只在 Unix 上跑：替身是 `sleep`。

use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use super::super::*;

/// 起一个睡 `seconds` 秒的命令。
fn sleeping(seconds: &str) -> Started {
    let mut command = Command::new("sleep");
    command.arg(seconds);
    start(command, None, |_| {}, false).expect("起得来")
}

#[test]
fn while_someone_is_asked_the_time_limit_does_not_run_out() {
    let started = sleeping("0.6");
    let wakes = Wakes::new();
    let hold = wakes.hold();
    hold(true);
    // 时限只有 50 毫秒：在等人，命令睡完照常结束。
    let finished = started
        .wait(Duration::from_millis(50), wakes)
        .expect("等得了");
    assert!(
        matches!(finished.ending, Ending::Exited(status) if status.success()),
        "{:?}",
        finished.ending
    );
}

#[test]
fn after_the_answer_the_rest_of_the_limit_runs_on() {
    let started = sleeping("30");
    let wakes = Wakes::new();
    let hold = wakes.hold();
    hold(true);
    let begin = Instant::now();
    let answered = thread::spawn(move || {
        thread::sleep(Duration::from_millis(400));
        hold(false);
    });
    let finished = started
        .wait(Duration::from_millis(100), wakes)
        .expect("等得了");
    answered.join().expect("答得了");
    assert!(
        matches!(finished.ending, Ending::TimedOut),
        "{:?}",
        finished.ending
    );
    assert!(
        begin.elapsed() >= Duration::from_millis(400),
        "等人的那一段不算：{:?}",
        begin.elapsed()
    );
}

#[test]
fn a_command_waiting_for_someone_is_not_moved_to_the_background() {
    let started = sleeping("0.6");
    let wakes = Wakes::new();
    let hold = wakes.hold();
    hold(true);
    let waited = started
        .wait_or_move(Duration::from_millis(50), wakes)
        .expect("等得了");
    assert!(
        matches!(
            waited,
            Waited::Finished(Finished {
                ending: Ending::Exited(_),
                ..
            })
        ),
        "在等人，到时也不挪"
    );
}

#[test]
fn a_stray_answer_before_any_question_changes_nothing() {
    let started = sleeping("30");
    let wakes = Wakes::new();
    let hold = wakes.hold();
    hold(false);
    let begin = Instant::now();
    let finished = started
        .wait(Duration::from_millis(100), wakes)
        .expect("等得了");
    assert!(matches!(finished.ending, Ending::TimedOut));
    assert!(begin.elapsed() < Duration::from_secs(20), "照旧到时就杀");
}

/// 跑到一半才开始等人：等之前走掉的那一段照算，只是等人的那一段不算。命令睡 4 秒、时限 1.5 秒，1 秒时开始等、3 秒时
/// 等完：到 3.5 秒就到时了，杀掉；等之前那 1 秒要是也不算，就会拖到 4.5 秒、命令先睡完了。命令睡完的那一刻卡在两者
/// 正中，两边各留 500 毫秒：CI 的 macOS 机器慢，原来只留 200 毫秒，等人的线程晚醒一点命令就先睡完了（M-7 上补）。
#[test]
fn the_time_before_the_question_still_counts() {
    let started = sleeping("4");
    let wakes = Wakes::new();
    let hold = wakes.hold();
    let asked = thread::spawn(move || {
        thread::sleep(Duration::from_millis(1000));
        hold(true);
        thread::sleep(Duration::from_millis(2000));
        hold(false);
    });
    let finished = started
        .wait(Duration::from_millis(1500), wakes)
        .expect("等得了");
    asked.join().expect("问得了");
    assert!(
        matches!(finished.ending, Ending::TimedOut),
        "{:?}",
        finished.ending
    );
}
