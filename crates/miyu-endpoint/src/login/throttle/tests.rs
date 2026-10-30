use super::*;

/// 从 `base` 起过 `seconds` 秒。
fn after(base: Instant, seconds: u64) -> Instant {
    base + Duration::from_secs(seconds)
}

#[test]
fn a_source_is_held_after_five_tries() {
    let throttle = Throttle::default();
    let base = Instant::now();
    for _ in 0..TRIES {
        throttle.admit_at("192.168.1.20", base).expect("还能试");
    }
    let wait = throttle
        .admit_at("192.168.1.20", after(base, 10))
        .unwrap_err();
    assert_eq!(wait, Duration::from_secs(50), "等到最早那一次满 60 秒");
    throttle
        .admit_at("192.168.1.21", base)
        .expect("别的来源照试");
}

#[test]
fn ports_and_one_ipv6_block_count_as_one_source() {
    for (first, second) in [
        ("192.168.1.20:50001", "192.168.1.20:50002"),
        (
            "[2001:db8:1:2::10]:443",
            "[2001:db8:1:2:aaaa:bbbb:cccc:dddd]:443",
        ),
        ("2001:db8:1:2::10", "2001:db8:1:2::11"),
        ("::ffff:192.168.1.20", "192.168.1.20"),
    ] {
        let throttle = Throttle::default();
        let base = Instant::now();
        for _ in 0..TRIES {
            throttle.admit_at(first, base).expect("还能试");
        }
        assert!(
            throttle.admit_at(second, base).is_err(),
            "{second} 和 {first} 算一份"
        );
    }
    let throttle = Throttle::default();
    let base = Instant::now();
    for _ in 0..TRIES {
        throttle.admit_at("2001:db8:1:2::10", base).expect("还能试");
    }
    throttle
        .admit_at("2001:db8:1:3::10", base)
        .expect("另一段 /64 是另一份");
}

#[test]
fn all_sources_together_are_held_after_thirty_tries() {
    let throttle = Throttle::default();
    let base = Instant::now();
    for index in 0..ALL_TRIES {
        throttle
            .admit_at(&format!("10.0.{}.{}", index / 200, index % 200), base)
            .expect("每个来源只试一次");
    }
    let wait = throttle.admit_at("10.9.9.9", after(base, 1)).unwrap_err();
    assert_eq!(wait, Duration::from_secs(59));
    throttle
        .admit_at("10.9.9.9", after(base, 60))
        .expect("满了窗口又能试");
}

#[test]
fn a_right_password_takes_its_try_back() {
    let throttle = Throttle::default();
    let base = Instant::now();
    for index in 0..ALL_TRIES * 2 {
        let source = format!("10.1.0.{index}");
        let at = throttle.admit_at(&source, base).expect("验对了的不占数");
        throttle.forgive(&source, at);
    }
}

#[test]
fn old_tries_and_their_sources_are_cleared() {
    let throttle = Throttle::default();
    let base = Instant::now();
    for index in 0..20 {
        throttle
            .admit_at(&format!("10.2.0.{index}"), base)
            .expect("还能试");
    }
    throttle
        .admit_at("10.3.0.1", after(base, 61))
        .expect("还能试");
    let counts = throttle.counts.lock().unwrap();
    assert_eq!(counts.by_source.len(), 1, "过了窗口的来源去掉了");
    assert_eq!(counts.all.len(), 1);
}
