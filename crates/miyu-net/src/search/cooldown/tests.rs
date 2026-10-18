//! 冷却的台阶、到点回来、成功清零。

use std::time::{Duration, Instant};

use super::Cooldowns;

fn minutes(n: u64) -> Duration {
    Duration::from_secs(n * 60)
}

#[test]
fn each_strike_climbs_the_ladder_up_to_the_top() {
    let mut cooldowns = Cooldowns::new(vec![minutes(30), minutes(60), minutes(120), minutes(360)]);
    let start = Instant::now();
    let mut now = start;
    let mut waits = Vec::new();
    for _ in 0..6 {
        let until = cooldowns.limited("exa", now);
        waits.push((until - now).as_secs() / 60);
        now = until;
    }
    assert_eq!(waits, [30, 60, 120, 360, 360, 360], "最多 6 小时");
}

#[test]
fn a_cooling_source_comes_back_at_its_time_and_a_success_clears_it() {
    let mut cooldowns = Cooldowns::new(vec![minutes(30), minutes(60)]);
    let now = Instant::now();
    let until = cooldowns.limited("exa", now);
    assert_eq!(cooldowns.until("exa", now), Some(until));
    assert_eq!(cooldowns.until("exa", now + minutes(29)), Some(until));
    assert_eq!(
        cooldowns.until("exa", now + minutes(30)),
        None,
        "到点自动回来"
    );
    assert_eq!(cooldowns.until("parallel", now), None, "各家各算");

    // 到点回来再被限：上一级台阶
    let again = cooldowns.limited("exa", now + minutes(30));
    assert_eq!(again - (now + minutes(30)), minutes(60));
    cooldowns.succeeded("exa");
    let fresh = cooldowns.limited("exa", now + minutes(100));
    assert_eq!(fresh - (now + minutes(100)), minutes(30), "成功一次清零");
}
