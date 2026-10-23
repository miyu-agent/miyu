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

/// 有会话索引的账号照它删没了的场所；没有索引的只照时间。
#[test]
fn the_start_sweep_prunes_every_account_and_trusts_only_a_readable_index() {
    let scratch = tempfile_dir();
    let env = miyu_store::env::Env {
        platform: miyu_store::env::Platform::current(),
        miyu_home: Some(scratch.join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    };
    let root = DataRoot::locate(&env).expect("找得到");
    let now = SystemTime::now();
    let (bridge, alice) = (
        AccountId::parse("bridge").expect("合写法"),
        AccountId::parse("alice").expect("合写法"),
    );
    // bridge 有索引（空的：一个会话都没有），群 1 那一层是新的也整层删。
    fs::create_dir_all(root.index(&bridge)).expect("建得了");
    let (index, _) = SessionIndex::open(&root.index(&bridge).join(FILE));
    drop(index);
    aged(&root.cache(&bridge).join("@qq_3agroup_3a1/kit/x"), 1, now);
    aged(&root.cache(&bridge).join("kit/old"), 31, now);
    // alice 没有索引：场所那一层照时间，新的留着。
    aged(&root.cache(&alice).join("@qq_3agroup_3a1/kit/x"), 1, now);
    aged(&root.cache(&alice).join("kit/old"), 31, now);
    let (removed, failed) = sweep_now(&root, now);
    assert_eq!((removed, failed), (3, 0));
    assert!(!root.cache(&bridge).join("@qq_3agroup_3a1").exists());
    assert!(
        root.cache(&alice).join("@qq_3agroup_3a1/kit/x").exists(),
        "不知道的不删"
    );
    assert!(!root.cache(&alice).join("kit/old").exists());
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
