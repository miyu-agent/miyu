//! 进程的用量：两种文件的读法；Linux 上真读得到，别的平台没有。

use super::*;

/// 一份真机的 `smaps_rollup`（Linux 6.x）。
const ROLLUP: &str = "\
55d0c3a00000-7ffd2b5ff000 ---p 00000000 00:00 0                          [rollup]
Rss:               48812 kB
Pss:               41230 kB
Pss_Dirty:         30100 kB
Pss_Anon:          29800 kB
Pss_File:          11430 kB
Pss_Shmem:             0 kB
Shared_Clean:      12000 kB
Shared_Dirty:          0 kB
Private_Clean:      6512 kB
Private_Dirty:     30300 kB
Referenced:        48000 kB
Anonymous:         29900 kB
LazyFree:              0 kB
AnonHugePages:         0 kB
Swap:                128 kB
SwapPss:             128 kB
Locked:                0 kB
";

#[test]
fn the_rollup_is_read_in_bytes() {
    assert_eq!(
        parse_rollup(ROLLUP),
        Some(Usage {
            rss: 48_812 * 1024,
            pss: Some(41_230 * 1024),
            private: Some((6_512 + 30_300) * 1024),
            anon: Some(29_900 * 1024),
            swap: Some(128 * 1024),
            footprint: None,
        })
    );
}

#[test]
fn something_that_is_not_a_rollup_reads_as_nothing() {
    assert_eq!(parse_rollup(""), None);
    assert_eq!(parse_rollup("Pss: 12 kB\n"), None, "没有 Rss 的不算");
    assert_eq!(parse_rollup("Rss: lots\n"), None);
}

#[test]
fn the_resident_size_comes_from_vmrss() {
    let status = "Name:\tmiyu\nVmPeak:\t  900000 kB\nVmRSS:\t   48812 kB\nThreads:\t9\n";
    assert_eq!(parse_status_rss(status), Some(48_812 * 1024));
    assert_eq!(parse_status_rss("Name:\tmiyu\n"), None);
}

#[test]
fn this_process_is_measured_on_linux_and_macos() {
    let usage = usage();
    let resident = resident();
    if cfg!(target_os = "linux") {
        let usage = usage.expect("Linux 上读得到 smaps_rollup");
        assert!(usage.rss > 0, "{usage:?}");
        assert!(usage.pss.is_some_and(|pss| pss > 0), "{usage:?}");
        assert!(
            usage.private.is_some_and(|private| private > 0),
            "{usage:?}"
        );
        assert_eq!(usage.footprint, None);
        assert!(resident.is_some_and(|rss| rss > 0));
    } else if cfg!(target_os = "macos") {
        let usage = usage.expect("macOS 上问得到内核");
        assert!(usage.rss > 0, "{usage:?}");
        assert!(
            usage.footprint.is_some_and(|footprint| footprint > 0),
            "{usage:?}"
        );
        assert_eq!(
            (usage.pss, usage.private, usage.anon, usage.swap),
            (None, None, None, None)
        );
        assert!(resident.is_some_and(|rss| rss > 0));
    } else {
        assert_eq!((usage, resident), (None, None));
    }
}

/// 量尺（施工 V-4，`cargo test -p miyu-heap --release -- --ignored --nocapture sampling_cost`）：采一次（`usage` 加堆的统计）、
/// 看一眼（`resident`）各要多久，印出来，不断言。
#[test]
#[ignore = "量尺：手动跑"]
fn sampling_cost() {
    let rounds = 1000_u32;
    let started = std::time::Instant::now();
    for _ in 0..rounds {
        std::hint::black_box((usage(), crate::stats()));
    }
    let sample = started.elapsed() / rounds;
    let started = std::time::Instant::now();
    for _ in 0..rounds {
        std::hint::black_box(resident());
    }
    let peek = started.elapsed() / rounds;
    println!("sample {sample:?} peek {peek:?} per call ({rounds} rounds)");
}
