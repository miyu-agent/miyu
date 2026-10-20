//! 读不等写（施工 V-14）：后台那一批写到一半（事务开着、还没提交）时，回合开始找记忆照样当场读完，看到的是上一次提交的。

use std::sync::mpsc;
use std::time::Duration;

use miyu_kernel::time::Timestamp;

use super::*;

/// 一个临时目录里的库；用完删掉。
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("miyu-recall-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).unwrap_or(());
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap_or(());
    }
}

#[test]
fn reading_does_not_wait_for_a_write_in_progress() {
    let scratch = Scratch::new("reading");
    let (index, _) = RecallIndex::open(&scratch.0.join("turns.db"));
    let at = Timestamp::from_unix_millis(1).expect("时刻");
    index.put("s1/1", "她记得你用 N 卡", at).expect("写进");
    let index = &index;
    let (opened, writing) = mpsc::channel();
    let (release, released) = mpsc::channel::<()>();
    let (found, read) = mpsc::channel();
    std::thread::scope(|scope| {
        scope.spawn(move || {
            index
                .write(|tx| {
                    tx.execute("INSERT INTO buried (key) VALUES ('s1/9')", [])?;
                    opened.send(()).expect("告诉读的");
                    // 等读的读完才提交：读的要是等写的锁，这里一直等到超时。
                    released.recv_timeout(Duration::from_secs(5)).unwrap_or(());
                    Ok(())
                })
                .expect("写");
        });
        writing.recv().expect("写的开了事务");
        scope.spawn(move || {
            let hits = index.search("N 卡", 5).expect("读");
            let buried = index.is_buried("s1/9").expect("读");
            found.send((hits.len(), buried)).expect("交回");
        });
        let got = read.recv_timeout(Duration::from_secs(2));
        release.send(()).expect("放写的提交");
        assert_eq!(
            got.ok(),
            Some((1, false)),
            "写到一半时当场读完，看到上一次提交的"
        );
    });
    assert!(index.is_buried("s1/9").expect("读"), "提交了以后读得到");
}
