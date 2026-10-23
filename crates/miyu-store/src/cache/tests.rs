use std::fs;
use std::time::{Duration, SystemTime};

use super::*;
use crate::test_support::Scratch;

const DAY: Duration = Duration::from_secs(24 * 60 * 60);

/// 在 `dir` 里放一个文件，修改时刻是 `days` 天前。
fn file_aged(dir: &Path, days: u64, now: SystemTime) {
    fs::create_dir_all(dir).expect("建得了");
    let file = dir.join("f");
    fs::write(&file, b"x").expect("写得进");
    let at = now - DAY * u32::try_from(days).expect("天数不大");
    fs::File::options()
        .write(true)
        .open(&file)
        .and_then(|opened| opened.set_modified(at))
        .expect("改得了时刻");
}

#[test]
fn a_month_old_cell_goes_and_a_fresh_one_stays() {
    let scratch = Scratch::new();
    let cache = scratch.path().join("cache");
    let now = SystemTime::now();
    file_aged(&cache.join("kit/old"), 31, now);
    file_aged(&cache.join("kit/new"), 29, now);
    // 一格里有旧有新的：照最新的算，留着。
    file_aged(&cache.join("~home/mixed/a"), 40, now);
    file_aged(&cache.join("~home/mixed/b"), 1, now);
    let (gone, failed) = prune(&cache, now, KEEP, &|_| true);
    assert!(failed.is_empty(), "{failed:?}");
    assert_eq!(gone, [cache.join("kit/old")]);
    assert!(!cache.join("kit/old").exists());
    assert!(cache.join("kit/new").exists());
    assert!(cache.join("~home/mixed").exists());
}

#[test]
fn a_venue_that_is_gone_goes_whole_and_a_live_one_is_pruned_like_the_rest() {
    let scratch = Scratch::new();
    let cache = scratch.path().join("cache");
    let now = SystemTime::now();
    file_aged(&cache.join("@qq_3agroup_3a1/kit/live"), 1, now);
    file_aged(&cache.join("@qq_3agroup_3a2/kit/old"), 31, now);
    file_aged(&cache.join("@qq_3agroup_3a2/kit/new"), 1, now);
    let one = VenueId::parse("qq:group:2").expect("合写法");
    let (gone, failed) = prune(&cache, now, KEEP, &|venue| *venue == one);
    assert!(failed.is_empty(), "{failed:?}");
    let mut gone = gone;
    gone.sort();
    assert_eq!(
        gone,
        [
            cache.join("@qq_3agroup_3a1"),
            cache.join("@qq_3agroup_3a2/kit/old")
        ],
        "没了的场所整层删，还在的照时间删"
    );
    assert!(cache.join("@qq_3agroup_3a2/kit/new").exists());
}

#[test]
fn a_missing_cache_does_nothing() {
    let scratch = Scratch::new();
    let (gone, failed) = prune(
        &scratch.path().join("none"),
        SystemTime::now(),
        KEEP,
        &|_| true,
    );
    assert!(gone.is_empty() && failed.is_empty());
}
