use std::fs;
use std::time::{Duration, SystemTime};

use super::*;

const DAY: Duration = Duration::from_secs(24 * 60 * 60);

/// 在 `dir` 里放一个 `days` 天前改过的文件。
fn aged(dir: &std::path::Path, days: u32, now: SystemTime) {
    fs::create_dir_all(dir).expect("建得了");
    let file = dir.join("f");
    fs::write(&file, b"x").expect("写得进");
    fs::File::options()
        .write(true)
        .open(&file)
        .and_then(|opened| opened.set_modified(now - DAY * days))
        .expect("改得了时刻");
}

/// 数据根 `scratch/data`。
fn root_in(scratch: &std::path::Path) -> DataRoot {
    let env = miyu_store::env::Env {
        platform: miyu_store::env::Platform::current(),
        miyu_home: Some(scratch.join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    };
    DataRoot::locate(&env).expect("找得到")
}

/// 有会话索引的照它删没了的场所：索引是空的（一个会话都没有），群 1 那一层是新的也整层删；老的照时间删。
#[test]
fn the_start_sweep_trusts_a_readable_index() {
    let scratch = tempfile_dir();
    let root = root_in(&scratch);
    let now = SystemTime::now();
    fs::create_dir_all(root.index()).expect("建得了");
    let (index, _) = SessionIndex::open(&root.index().join(FILE));
    drop(index);
    aged(&root.cache().join("@qq_3agroup_3a1/kit/x"), 1, now);
    aged(&root.cache().join("kit/old"), 31, now);
    assert_eq!(sweep_now(&root, now), (2, 0));
    assert!(!root.cache().join("@qq_3agroup_3a1").exists());
    assert!(!root.cache().join("kit/old").exists());
    fs::remove_dir_all(scratch).expect("删得掉");
}

/// 没有索引的只照时间：场所那一层新的留着（不知道的不删）。
#[test]
fn without_an_index_only_age_counts() {
    let scratch = tempfile_dir();
    let root = root_in(&scratch);
    let now = SystemTime::now();
    aged(&root.cache().join("@qq_3agroup_3a1/kit/x"), 1, now);
    aged(&root.cache().join("kit/old"), 31, now);
    assert_eq!(sweep_now(&root, now), (1, 0));
    assert!(
        root.cache().join("@qq_3agroup_3a1/kit/x").exists(),
        "不知道的不删"
    );
    assert!(!root.cache().join("kit/old").exists());
    fs::remove_dir_all(scratch).expect("删得掉");
}

/// 一个新的空目录。
fn tempfile_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "miyu-core-cache-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos())
    ));
    fs::create_dir_all(&dir).expect("建得了");
    dir
}
