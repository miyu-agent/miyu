//! 新建的段文件只给自己读写（施工 AUD-5）：会话日志里是人和她说的话，同数据根里别的日志（`journal.jsonl`、运行日志）一样
//! 是 0600。头一段、写满了换出来的下一段都是；已经在的段不动它的权限。

use std::os::unix::fs::PermissionsExt;

use super::*;

/// 这个文件的权限位。
fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn new_segments_are_private() {
    let scratch = Scratch::new();
    let dir = dir(&scratch);
    // 上限很小：写两批就换一段。
    let mut log = SessionLog::create(&dir, 1).unwrap();
    log.append(&said_range(1, 2)).unwrap();
    log.append(&said_range(3, 4)).unwrap();
    let names = segment_names(&dir);
    assert_eq!(names.len(), 2, "{names:?}");
    for name in names {
        assert_eq!(mode(&dir.join(&name)), 0o600, "{name}");
    }
}

#[test]
fn a_segment_already_there_keeps_its_mode() {
    let scratch = Scratch::new();
    let dir = dir(&scratch);
    let mut log = SessionLog::create(&dir, SEGMENT_LIMIT).unwrap();
    log.append(&said_range(1, 2)).unwrap();
    drop(log);
    let only = dir.join(&segment_names(&dir)[0]);
    fs::set_permissions(&only, fs::Permissions::from_mode(0o640)).unwrap();
    let (mut log, _) = SessionLog::open(&dir, SEGMENT_LIMIT).unwrap();
    log.append(&said_range(3, 3)).unwrap();
    assert_eq!(mode(&only), 0o640, "打开接着写的不改权限");
}
